use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::model::{
    CommandConfig, CommandOutputMode, FileLinesConfig, HistoryEntry, IPythonConfig, ProviderConfig,
    ProviderKind, SqliteQueryConfig,
};

const MAX_STORED_ENTRY_BYTES: usize = 4 * 1024 * 1024;

pub const SQLITE_HELPER: &str = r#"
import json
import sqlite3
import sys

path = sys.argv[1]
query = sys.argv[2]
uri = "file:{}?mode=ro".format(path)
conn = sqlite3.connect(uri, uri=True)
cursor = conn.execute(query)
for row in cursor:
    values = []
    for value in row:
        if value is None:
            values.append(None)
        else:
            values.append(str(value))
    print(json.dumps({"values": values}, ensure_ascii=False))
"#;

#[derive(Clone, Debug)]
pub struct CommandInvocation {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct SqliteRow {
    values: Vec<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct CommandJsonRow {
    text: Option<String>,
    preview: Option<String>,
    score_hint: Option<i64>,
}

pub fn provider_requires_full_hd(config: &ProviderConfig) -> bool {
    matches!(
        config.kind,
        ProviderKind::FileLines(_) | ProviderKind::SqliteQuery(_) | ProviderKind::IPython(_)
    )
}

pub fn provider_requires_run_commands(config: &ProviderConfig) -> bool {
    matches!(
        config.kind,
        ProviderKind::Command(_) | ProviderKind::SqliteQuery(_) | ProviderKind::IPython(_)
    )
}

pub fn load_file_provider(config: &ProviderConfig) -> Result<Vec<HistoryEntry>, String> {
    let file_config = match &config.kind {
        ProviderKind::FileLines(file_config) => file_config,
        _ => return Err("Internal error: expected file_lines provider".to_owned()),
    };

    let path = config_path_to_wasi(&file_config.path)?;
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", file_config.path))?;
    let mut lines = contents
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if file_config.reverse {
        lines.reverse();
    }

    let entries = lines
        .into_iter()
        .enumerate()
        .map(|(index, text)| HistoryEntry {
            preview: None,
            text,
            score_hint: (file_config.limit.saturating_sub(index)) as i64,
        })
        .collect::<Vec<_>>();

    Ok(finalize_entries(
        entries,
        file_config.dedupe,
        file_config.limit,
    ))
}

pub fn build_command_invocation(config: &ProviderConfig) -> Result<CommandInvocation, String> {
    match &config.kind {
        ProviderKind::Command(command_config) => Ok(CommandInvocation {
            argv: std::iter::once(command_config.command.clone())
                .chain(command_config.args.iter().cloned())
                .collect(),
            cwd: command_config
                .cwd
                .as_ref()
                .map(|cwd| expand_host_path(cwd))
                .transpose()?,
            env: command_config.env.clone(),
        }),
        ProviderKind::SqliteQuery(sqlite_config) => build_sqlite_invocation(sqlite_config),
        ProviderKind::IPython(ipython_config) => {
            let sqlite_config = ipython_to_sqlite(ipython_config);
            build_sqlite_invocation(&sqlite_config)
        }
        ProviderKind::FileLines(_) => {
            Err("Internal error: file_lines is not a command provider".to_owned())
        }
    }
}

pub fn parse_command_output(
    config: &ProviderConfig,
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Vec<HistoryEntry>, String> {
    if exit_code != Some(0) {
        let stderr = String::from_utf8_lossy(stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(stdout).trim().to_owned();
        return Err(if stderr.is_empty() {
            if stdout.is_empty() {
                match &config.kind {
                    ProviderKind::SqliteQuery(sqlite_config) => format!(
                        "Provider '{}' failed with exit code {:?}. Check SQLite path and query: {}",
                        config.name, exit_code, sqlite_config.path
                    ),
                    ProviderKind::IPython(ipython_config) => format!(
                        "Provider '{}' failed with exit code {:?}. Check IPython history path: {}",
                        config.name, exit_code, ipython_config.path
                    ),
                    _ => format!(
                        "Provider '{}' failed with exit code {:?}",
                        config.name, exit_code
                    ),
                }
            } else {
                format!("Provider '{}' failed: {}", config.name, stdout)
            }
        } else {
            format!("Provider '{}' failed: {}", config.name, stderr)
        });
    }

    match &config.kind {
        ProviderKind::Command(command_config) => parse_line_output(config, command_config, stdout),
        ProviderKind::SqliteQuery(sqlite_config) => {
            parse_sqlite_output(config, sqlite_config, stdout)
        }
        ProviderKind::IPython(ipython_config) => {
            let sqlite_config = ipython_to_sqlite(ipython_config);
            parse_sqlite_output(config, &sqlite_config, stdout)
        }
        ProviderKind::FileLines(_) => {
            Err("Internal error: file_lines does not use command output".to_owned())
        }
    }
}

fn parse_line_output(
    config: &ProviderConfig,
    command_config: &CommandConfig,
    stdout: &[u8],
) -> Result<Vec<HistoryEntry>, String> {
    let output = String::from_utf8(stdout.to_vec())
        .map_err(|error| format!("Provider '{}' returned invalid UTF-8: {error}", config.name))?;
    let entries = match command_config.output_mode {
        CommandOutputMode::Lines => output
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(index, text)| HistoryEntry {
                text: text.to_owned(),
                preview: None,
                score_hint: (command_config.limit.saturating_sub(index)) as i64,
            })
            .collect::<Vec<_>>(),
        CommandOutputMode::Json => parse_json_lines_output(config, command_config, &output)?,
    };

    Ok(finalize_entries(
        entries,
        command_config.dedupe,
        command_config.limit,
    ))
}

fn parse_json_lines_output(
    config: &ProviderConfig,
    command_config: &CommandConfig,
    output: &str,
) -> Result<Vec<HistoryEntry>, String> {
    let mut entries = Vec::new();

    for (index, line) in output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let row: CommandJsonRow = serde_json::from_str(line).map_err(|error| {
            format!(
                "Provider '{}' returned invalid command_json rows: {error}",
                config.name
            )
        })?;
        let text = row.text.ok_or_else(|| {
            format!(
                "Provider '{}' returned a command_json row without `text`",
                config.name
            )
        })?;
        if text.trim().is_empty() {
            continue;
        }
        let preview = row.preview.filter(|preview| preview != &text);
        let score_hint = row
            .score_hint
            .unwrap_or((command_config.limit.saturating_sub(index)) as i64);

        entries.push(HistoryEntry {
            text,
            preview,
            score_hint,
        });
    }

    Ok(entries)
}

fn build_sqlite_invocation(sqlite_config: &SqliteQueryConfig) -> Result<CommandInvocation, String> {
    let path = expand_host_path(&sqlite_config.path)?;
    let wasi_path = config_path_to_wasi(&sqlite_config.path)?;
    let metadata = fs::metadata(&wasi_path).map_err(|error| {
        format!(
            "Failed to read SQLite database {}: {error}",
            sqlite_config.path
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "SQLite database path is not a file: {}",
            sqlite_config.path
        ));
    }
    Ok(CommandInvocation {
        argv: vec![
            "python3".to_owned(),
            "-c".to_owned(),
            SQLITE_HELPER.to_owned(),
            path.to_string_lossy().to_string(),
            sqlite_config.query.clone(),
        ],
        cwd: None,
        env: BTreeMap::new(),
    })
}

fn ipython_to_sqlite(config: &IPythonConfig) -> SqliteQueryConfig {
    let query = config.query_override.clone().unwrap_or_else(|| {
        format!(
            "SELECT COALESCE(source_raw, source), printf('%d:%d', session, line), session \
             FROM history \
             WHERE COALESCE(source_raw, source) IS NOT NULL \
             ORDER BY session DESC, line DESC \
             LIMIT {}",
            config.limit
        )
    });
    SqliteQueryConfig {
        path: config.path.clone(),
        query,
        text_column: 0,
        preview_column: None,
        timestamp_column: Some(1),
        limit: config.limit,
        dedupe: config.dedupe,
    }
}

fn parse_sqlite_output(
    config: &ProviderConfig,
    sqlite_config: &SqliteQueryConfig,
    stdout: &[u8],
) -> Result<Vec<HistoryEntry>, String> {
    let output = String::from_utf8(stdout.to_vec())
        .map_err(|error| format!("Provider '{}' returned invalid UTF-8: {error}", config.name))?;
    let mut entries = Vec::new();

    for (index, line) in output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let row: SqliteRow = serde_json::from_str(line).map_err(|error| {
            format!(
                "Provider '{}' returned invalid SQLite JSON rows: {error}",
                config.name
            )
        })?;
        let text = get_column(&row.values, sqlite_config.text_column).ok_or_else(|| {
            format!(
                "Provider '{}' missing text column {}",
                config.name, sqlite_config.text_column
            )
        })?;
        let preview = sqlite_config
            .preview_column
            .and_then(|column| get_column(&row.values, column).map(str::to_owned))
            .filter(|preview| preview != text);

        entries.push(HistoryEntry {
            text: text.to_owned(),
            preview,
            score_hint: (sqlite_config.limit.saturating_sub(index)) as i64,
        });
    }

    Ok(finalize_entries(
        entries,
        sqlite_config.dedupe,
        sqlite_config.limit,
    ))
}

fn get_column(values: &[Option<String>], index: usize) -> Option<&str> {
    values.get(index).and_then(|value| value.as_deref())
}

fn finalize_entries(
    mut entries: Vec<HistoryEntry>,
    dedupe: bool,
    limit: usize,
) -> Vec<HistoryEntry> {
    if dedupe {
        let mut seen = HashSet::new();
        entries.retain(|entry| seen.insert(entry.text.clone()));
    }
    if entries.len() > limit {
        entries.truncate(limit);
    }
    let mut total_bytes = 0usize;
    entries.retain(|entry| {
        let entry_bytes = entry.text.len()
            + entry
                .preview
                .as_ref()
                .map(|preview| preview.len())
                .unwrap_or(0);
        if total_bytes + entry_bytes > MAX_STORED_ENTRY_BYTES {
            return false;
        }
        total_bytes += entry_bytes;
        true
    });
    entries
}

fn config_path_to_wasi(path: &str) -> Result<PathBuf, String> {
    let host_path = expand_host_path(path)?;
    if !host_path.is_absolute() {
        return Err(format!(
            "Expected absolute host path after expansion: {path}"
        ));
    }
    let relative = host_path
        .strip_prefix(Path::new("/"))
        .map_err(|_| format!("Failed to translate host path into /host mount: {path}"))?;
    Ok(Path::new("/host").join(relative))
}

fn expand_host_path(path: &str) -> Result<PathBuf, String> {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = env::var("HOME").map_err(|_| "HOME is not set".to_owned())?;
        return Ok(Path::new(&home).join(rest));
    }
    if path == "~" {
        let home = env::var("HOME").map_err(|_| "HOME is not set".to_owned())?;
        return Ok(PathBuf::from(home));
    }

    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        return Ok(candidate);
    }

    let base = env::var("PWD")
        .map(PathBuf::from)
        .or_else(|_| env::current_dir().map_err(|error| error.to_string()))?;
    Ok(base.join(candidate))
}

#[allow(dead_code)]
fn _assert_send_sync_usage(_config: &FileLinesConfig) {}

#[cfg(test)]
mod tests {
    use super::parse_command_output;
    use crate::model::{CommandConfig, CommandOutputMode, ProviderConfig, ProviderKind};
    use std::collections::BTreeMap;

    fn command_provider(output_mode: CommandOutputMode) -> ProviderConfig {
        ProviderConfig {
            name: "Test Command".to_owned(),
            kind: ProviderKind::Command(CommandConfig {
                command: "python3".to_owned(),
                args: Vec::new(),
                cwd: None,
                env: BTreeMap::new(),
                output_mode,
                limit: 5000,
                dedupe: false,
            }),
        }
    }

    #[test]
    fn parses_command_json_rows_into_multiline_entries() {
        let config = command_provider(CommandOutputMode::Json);
        let stdout = br#"{"text":"first line\nsecond line","preview":"full preview\nwith two lines","score_hint":42}
{"text":"single line"}
"#;

        let entries = parse_command_output(&config, Some(0), stdout, b"")
            .expect("command_json output should parse");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "first line\nsecond line");
        assert_eq!(
            entries[0].preview.as_deref(),
            Some("full preview\nwith two lines")
        );
        assert_eq!(entries[0].score_hint, 42);
        assert_eq!(entries[1].text, "single line");
        assert!(entries[1].preview.is_none());
    }

    #[test]
    fn errors_when_command_json_row_is_missing_text() {
        let config = command_provider(CommandOutputMode::Json);
        let stdout = br#"{"preview":"missing text"}"#;

        let error = parse_command_output(&config, Some(0), stdout, b"")
            .expect_err("row without text should fail");

        assert!(error.contains("without `text`"));
    }
}
