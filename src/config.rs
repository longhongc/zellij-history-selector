use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    AppConfig, CommandConfig, CommandOutputMode, DefaultMode, FileLinesConfig, IPythonConfig,
    ProviderConfig, ProviderKind, SqliteQueryConfig,
};

pub fn parse_config(raw: BTreeMap<String, String>) -> Result<AppConfig, String> {
    let mut default_mode = match raw
        .get("default_mode")
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("execute") => DefaultMode::Execute,
        Some("copy") => DefaultMode::Copy,
        _ => DefaultMode::Insert,
    };

    if parse_bool(raw.get("execute_on_select"), false)? {
        default_mode = DefaultMode::Execute;
    }
    let max_results = parse_usize(raw.get("max_results"), 500)?;
    let preview_lines = parse_usize(raw.get("preview_lines"), 10)?;
    let case_sensitive = parse_bool(raw.get("case_sensitive"), false)?;
    let providers = parse_providers(&raw)?;

    Ok(AppConfig {
        default_mode,
        max_results,
        preview_lines,
        case_sensitive,
        providers,
    })
}

fn parse_providers(raw: &BTreeMap<String, String>) -> Result<Vec<ProviderConfig>, String> {
    if raw.contains_key("providers") {
        return parse_namespaced_providers(raw);
    }
    if !collect_namespaced_provider_ids(raw).is_empty() {
        return Err(
            "Found namespaced provider keys but no `providers` order list. Add `providers \"id1,id2\"` or keep using legacy keys like provider_1_type."
                .to_owned(),
        );
    }
    parse_indexed_providers(raw)
}

fn parse_namespaced_providers(
    raw: &BTreeMap<String, String>,
) -> Result<Vec<ProviderConfig>, String> {
    let provider_ids = parse_provider_ids(
        raw.get("providers")
            .ok_or_else(|| "Missing `providers` config key.".to_owned())?,
    )?;
    let listed_provider_ids = provider_ids.iter().cloned().collect::<BTreeSet<_>>();
    let declared_provider_ids = collect_namespaced_provider_ids(raw);
    let unlisted_ids = declared_provider_ids
        .difference(&listed_provider_ids)
        .cloned()
        .collect::<Vec<_>>();
    if !unlisted_ids.is_empty() {
        return Err(format!(
            "Found provider.* keys for ids not listed in `providers`: {}",
            unlisted_ids.join(", ")
        ));
    }

    let mut providers = Vec::with_capacity(provider_ids.len());
    for provider_id in provider_ids {
        let prefix = format!("provider.{provider_id}.");
        providers.push(parse_provider_from_prefix(
            raw,
            &prefix,
            provider_id.clone(),
            &format!("provider.{provider_id}"),
        )?);
    }
    Ok(providers)
}

fn parse_indexed_providers(raw: &BTreeMap<String, String>) -> Result<Vec<ProviderConfig>, String> {
    let provider_indices = collect_provider_indices(raw);
    if provider_indices.is_empty() {
        return Err(
            "No providers configured. Use `providers \"id1,id2\"` with keys like `provider.shell.type`, or legacy keys like provider_1_type."
                .to_owned(),
        );
    }

    let mut providers = Vec::with_capacity(provider_indices.len());
    for provider_index in provider_indices {
        let prefix = format!("provider_{provider_index}_");
        providers.push(parse_provider_from_prefix(
            raw,
            &prefix,
            format!("Provider {provider_index}"),
            &format!("provider_{provider_index}"),
        )?);
    }
    Ok(providers)
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

fn collect_namespaced_provider_ids(raw: &BTreeMap<String, String>) -> BTreeSet<String> {
    raw.keys()
        .filter_map(|key| {
            let rest = key.strip_prefix("provider.")?;
            let (provider_id, _field) = rest.split_once('.')?;
            (!provider_id.is_empty()).then(|| provider_id.to_owned())
        })
        .collect()
}

fn parse_provider_ids(raw: &str) -> Result<Vec<String>, String> {
    let mut provider_ids = Vec::new();
    let mut seen = BTreeSet::new();

    for provider_id in raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !is_valid_provider_id(provider_id) {
            return Err(format!(
                "Invalid provider id `{provider_id}` in `providers`. Use only letters, numbers, `_`, and `-`."
            ));
        }
        if !seen.insert(provider_id.to_owned()) {
            return Err(format!(
                "Duplicate provider id `{provider_id}` in `providers`."
            ));
        }
        provider_ids.push(provider_id.to_owned());
    }

    if provider_ids.is_empty() {
        return Err("`providers` must list at least one provider id.".to_owned());
    }

    Ok(provider_ids)
}

fn is_valid_provider_id(provider_id: &str) -> bool {
    provider_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
}

fn parse_provider_from_prefix(
    raw: &BTreeMap<String, String>,
    prefix: &str,
    default_name: String,
    provider_ref: &str,
) -> Result<ProviderConfig, String> {
    let type_key = format!("{prefix}type");
    let name_key = format!("{prefix}name");
    let provider_type = raw
        .get(&type_key)
        .ok_or_else(|| format!("Missing required config key: {type_key} ({provider_ref})"))?
        .trim()
        .to_ascii_lowercase();
    let name = raw.get(&name_key).cloned().unwrap_or(default_name);

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
        "command" | "command_lines" => ProviderKind::Command(CommandConfig {
            command: required_string(raw, &format!("{prefix}command"))?,
            args: parse_args(raw.get(&format!("{prefix}args")))?,
            cwd: raw.get(&format!("{prefix}cwd")).cloned(),
            env: collect_env(raw, prefix),
            output_mode: CommandOutputMode::Lines,
            limit: parse_usize(raw.get(&format!("{prefix}limit")), 5000)?,
            dedupe: parse_bool(raw.get(&format!("{prefix}dedupe")), false)?,
        }),
        "command_json" => ProviderKind::Command(CommandConfig {
            command: required_string(raw, &format!("{prefix}command"))?,
            args: parse_args(raw.get(&format!("{prefix}args")))?,
            cwd: raw.get(&format!("{prefix}cwd")).cloned(),
            env: collect_env(raw, prefix),
            output_mode: CommandOutputMode::Json,
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
                "Unsupported provider type '{other}' in {provider_ref}.type"
            ));
        }
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
        Some(value) => shell_words::split(value)
            .map_err(|error| format!("Failed to parse command args '{value}': {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_config;
    use crate::model::{CommandOutputMode, DefaultMode, ProviderKind};
    use std::collections::BTreeMap;

    #[test]
    fn parses_namespaced_provider_config_in_declared_order() {
        let raw = BTreeMap::from([
            ("providers".to_owned(), "custom,bash".to_owned()),
            (
                "provider.custom.type".to_owned(),
                "command_lines".to_owned(),
            ),
            ("provider.custom.command".to_owned(), "python3".to_owned()),
            ("provider.custom.args".to_owned(), "-m exporter".to_owned()),
            ("provider.custom.env_MODE".to_owned(), "test".to_owned()),
            ("provider.bash.type".to_owned(), "file_lines".to_owned()),
            ("provider.bash.name".to_owned(), "Bash".to_owned()),
            (
                "provider.bash.path".to_owned(),
                "~/.bash_history".to_owned(),
            ),
            ("provider.bash.limit".to_owned(), "100".to_owned()),
        ]);

        let parsed = parse_config(raw).expect("config should parse");
        assert_eq!(parsed.providers.len(), 2);
        assert_eq!(parsed.providers[0].name, "custom");
        assert_eq!(parsed.providers[1].name, "Bash");
        match &parsed.providers[0].kind {
            ProviderKind::Command(config) => {
                assert_eq!(config.args, vec!["-m", "exporter"]);
                assert_eq!(config.env.get("MODE"), Some(&"test".to_owned()));
            }
            other => panic!("unexpected provider kind: {other:?}"),
        }
        match &parsed.providers[1].kind {
            ProviderKind::FileLines(config) => assert_eq!(config.limit, 100),
            other => panic!("unexpected provider kind: {other:?}"),
        }
    }

    #[test]
    fn errors_on_namespaced_provider_keys_without_provider_order_list() {
        let raw = BTreeMap::from([
            ("provider.shell.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.shell.path".to_owned(),
                "~/.zsh_history".to_owned(),
            ),
        ]);

        let error = parse_config(raw).expect_err("config should fail");
        assert!(error.contains("Found namespaced provider keys"));
        assert!(error.contains("providers"));
    }

    #[test]
    fn errors_on_unlisted_namespaced_provider_ids() {
        let raw = BTreeMap::from([
            ("providers".to_owned(), "shell".to_owned()),
            ("provider.shell.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.shell.path".to_owned(),
                "~/.zsh_history".to_owned(),
            ),
            ("provider.extra.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.extra.path".to_owned(),
                "~/.bash_history".to_owned(),
            ),
        ]);

        let error = parse_config(raw).expect_err("config should fail");
        assert!(error.contains("not listed in `providers`"));
        assert!(error.contains("extra"));
    }

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
                assert!(matches!(config.output_mode, CommandOutputMode::Lines));
            }
            other => panic!("unexpected provider kind: {other:?}"),
        }
    }

    #[test]
    fn parses_command_json_provider_config() {
        let raw = BTreeMap::from([
            ("providers".to_owned(), "copyq".to_owned()),
            ("provider.copyq.type".to_owned(), "command_json".to_owned()),
            ("provider.copyq.name".to_owned(), "CopyQ".to_owned()),
            ("provider.copyq.command".to_owned(), "python3".to_owned()),
            (
                "provider.copyq.args".to_owned(),
                "/tmp/export_copyq_json.py".to_owned(),
            ),
            ("provider.copyq.limit".to_owned(), "100".to_owned()),
        ]);

        let parsed = parse_config(raw).expect("config should parse");
        match &parsed.providers[0].kind {
            ProviderKind::Command(config) => {
                assert!(matches!(config.output_mode, CommandOutputMode::Json));
                assert_eq!(config.limit, 100);
            }
            other => panic!("unexpected provider kind: {other:?}"),
        }
    }

    #[test]
    fn parses_copy_default_mode() {
        let raw = BTreeMap::from([
            ("default_mode".to_owned(), "copy".to_owned()),
            ("providers".to_owned(), "shell".to_owned()),
            ("provider.shell.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.shell.path".to_owned(),
                "~/.bash_history".to_owned(),
            ),
        ]);

        let parsed = parse_config(raw).expect("config should parse");
        assert!(matches!(parsed.default_mode, DefaultMode::Copy));
    }

    #[test]
    fn execute_on_select_maps_to_execute_mode_for_compatibility() {
        let raw = BTreeMap::from([
            ("execute_on_select".to_owned(), "true".to_owned()),
            ("providers".to_owned(), "shell".to_owned()),
            ("provider.shell.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.shell.path".to_owned(),
                "~/.bash_history".to_owned(),
            ),
        ]);

        let parsed = parse_config(raw).expect("config should parse");
        assert!(matches!(parsed.default_mode, DefaultMode::Execute));
    }
}
