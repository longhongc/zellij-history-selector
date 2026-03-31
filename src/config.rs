use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    AppConfig, CommandConfig, DefaultMode, FileLinesConfig, IPythonConfig, ProviderConfig,
    ProviderKind, SplitMode, SqliteQueryConfig,
};

pub fn parse_config(raw: BTreeMap<String, String>) -> Result<AppConfig, String> {
    let default_mode = match raw
        .get("default_mode")
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("execute") => DefaultMode::Execute,
        Some("append") => DefaultMode::Append,
        _ => DefaultMode::Insert,
    };

    let execute_on_select = parse_bool(raw.get("execute_on_select"), false)?;
    let max_results = parse_usize(raw.get("max_results"), 500)?;
    let preview_lines = parse_usize(raw.get("preview_lines"), 12)?;
    let case_sensitive = parse_bool(raw.get("case_sensitive"), false)?;

    let provider_indices = collect_provider_indices(&raw);
    if provider_indices.is_empty() {
        return Err(
            "No providers configured. Use flat keys like provider_1_type, provider_1_name, provider_1_path."
                .to_owned(),
        );
    }

    let mut providers = Vec::new();
    for provider_index in provider_indices {
        providers.push(parse_provider(&raw, provider_index)?);
    }

    Ok(AppConfig {
        default_mode,
        execute_on_select,
        max_results,
        preview_lines,
        case_sensitive,
        providers,
    })
}

fn collect_provider_indices(raw: &BTreeMap<String, String>) -> BTreeSet<usize> {
    let mut indices = BTreeSet::new();
    for key in raw.keys() {
        if let Some(rest) = key.strip_prefix("provider_") {
            let mut parts = rest.splitn(2, '_');
            if let Some(index) = parts.next().and_then(|part| part.parse::<usize>().ok()) {
                indices.insert(index);
            }
        }
    }
    indices
}

fn parse_provider(raw: &BTreeMap<String, String>, index: usize) -> Result<ProviderConfig, String> {
    let prefix = format!("provider_{index}_");
    let type_key = format!("{prefix}type");
    let name_key = format!("{prefix}name");
    let provider_type = raw
        .get(&type_key)
        .ok_or_else(|| format!("Missing required config key: {type_key}"))?
        .trim()
        .to_ascii_lowercase();
    let name = raw
        .get(&name_key)
        .cloned()
        .unwrap_or_else(|| format!("Provider {index}"));

    let kind = match provider_type.as_str() {
        "file_lines" => ProviderKind::FileLines(FileLinesConfig {
            path: required_string(raw, &format!("{prefix}path"))?,
            reverse: parse_bool(raw.get(&format!("{prefix}reverse")), true)?,
            dedupe: parse_bool(raw.get(&format!("{prefix}dedupe")), false)?,
            limit: parse_usize(raw.get(&format!("{prefix}limit")), 5000)?,
        }),
        "sqlite_query" => ProviderKind::SqliteQuery(SqliteQueryConfig {
            path: required_string(raw, &format!("{prefix}path"))?,
            query: required_string(raw, &format!("{prefix}query"))?,
            text_column: parse_usize(raw.get(&format!("{prefix}text_column")), 0)?,
            preview_column: parse_optional_usize(raw.get(&format!("{prefix}preview_column")))?,
            timestamp_column: parse_optional_usize(raw.get(&format!("{prefix}timestamp_column")))?,
            limit: parse_usize(raw.get(&format!("{prefix}limit")), 5000)?,
            dedupe: parse_bool(raw.get(&format!("{prefix}dedupe")), false)?,
        }),
        "command" => ProviderKind::Command(CommandConfig {
            command: required_string(raw, &format!("{prefix}command"))?,
            args: parse_args(raw.get(&format!("{prefix}args")))?,
            cwd: raw.get(&format!("{prefix}cwd")).cloned(),
            env: collect_env(raw, &prefix),
            split_mode: SplitMode::Lines,
            limit: parse_usize(raw.get(&format!("{prefix}limit")), 5000)?,
            dedupe: parse_bool(raw.get(&format!("{prefix}dedupe")), false)?,
        }),
        "ipython" => ProviderKind::IPython(IPythonConfig {
            path: required_string(raw, &format!("{prefix}path"))?,
            query_override: raw.get(&format!("{prefix}query")).cloned(),
            limit: parse_usize(raw.get(&format!("{prefix}limit")), 5000)?,
            dedupe: parse_bool(raw.get(&format!("{prefix}dedupe")), false)?,
        }),
        other => {
            return Err(format!(
                "Unsupported provider type '{other}' in provider_{index}_type"
            ))
        },
    };

    Ok(ProviderConfig { name, kind })
}

fn collect_env(raw: &BTreeMap<String, String>, prefix: &str) -> BTreeMap<String, String> {
    let env_prefix = format!("{prefix}env_");
    raw.iter()
        .filter_map(|(key, value)| {
            key.strip_prefix(&env_prefix)
                .map(|env_key| (env_key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn required_string(raw: &BTreeMap<String, String>, key: &str) -> Result<String, String> {
    raw.get(key)
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("Missing required config key: {key}"))
}

fn parse_bool(value: Option<&String>, default: bool) -> Result<bool, String> {
    match value.map(|value| value.trim().to_ascii_lowercase()) {
        None => Ok(default),
        Some(value) if value == "true" => Ok(true),
        Some(value) if value == "false" => Ok(false),
        Some(value) => Err(format!("Invalid boolean value: {value}")),
    }
}

fn parse_usize(value: Option<&String>, default: usize) -> Result<usize, String> {
    match value {
        None => Ok(default),
        Some(value) => value
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("Invalid integer value: {value}")),
    }
}

fn parse_optional_usize(value: Option<&String>) -> Result<Option<usize>, String> {
    value
        .map(|value| {
            value
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("Invalid integer value: {value}"))
        })
        .transpose()
}

fn parse_args(value: Option<&String>) -> Result<Vec<String>, String> {
    match value {
        None => Ok(Vec::new()),
        Some(value) => shell_words::split(value).map_err(|error| {
            format!("Failed to parse command args '{value}': {error}")
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_config;
    use crate::model::ProviderKind;
    use std::collections::BTreeMap;

    #[test]
    fn parses_flat_provider_config() {
        let raw = BTreeMap::from([
            ("provider_1_type".to_owned(), "file_lines".to_owned()),
            ("provider_1_name".to_owned(), "Shell".to_owned()),
            ("provider_1_path".to_owned(), "~/.zsh_history".to_owned()),
            ("provider_1_limit".to_owned(), "100".to_owned()),
            ("provider_2_type".to_owned(), "command".to_owned()),
            ("provider_2_name".to_owned(), "Custom".to_owned()),
            ("provider_2_command".to_owned(), "python3".to_owned()),
            ("provider_2_args".to_owned(), "-m exporter".to_owned()),
            ("provider_2_env_MODE".to_owned(), "test".to_owned()),
        ]);

        let parsed = parse_config(raw).expect("config should parse");
        assert_eq!(parsed.providers.len(), 2);
        match &parsed.providers[0].kind {
            ProviderKind::FileLines(config) => assert_eq!(config.limit, 100),
            other => panic!("unexpected provider kind: {other:?}"),
        }
        match &parsed.providers[1].kind {
            ProviderKind::Command(config) => {
                assert_eq!(config.args, vec!["-m", "exporter"]);
                assert_eq!(config.env.get("MODE"), Some(&"test".to_owned()));
            },
            other => panic!("unexpected provider kind: {other:?}"),
        }
    }
}
