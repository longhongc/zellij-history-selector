use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    AppConfig, CommandConfig, CommandOutputMode, DefaultMode, FileLinesConfig, IPythonConfig,
    ProviderConfig, ProviderKind, SqliteQueryConfig,
};
use crate::provider::{provider_requires_full_hd, provider_requires_run_commands};

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
    let selected_profile = raw
        .get("profile")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let ProviderSelection {
        active_providers: providers,
        all_declared_providers,
        active_profile,
    } = parse_providers(&raw, selected_profile.as_deref())?;
    let requires_run_commands = all_declared_providers
        .iter()
        .any(provider_requires_run_commands);
    let requires_full_hd_access = all_declared_providers.iter().any(provider_requires_full_hd);

    Ok(AppConfig {
        active_profile,
        default_mode,
        max_results,
        preview_lines,
        case_sensitive,
        requires_run_commands,
        requires_full_hd_access,
        providers,
    })
}

struct ProviderSelection {
    active_providers: Vec<ProviderConfig>,
    all_declared_providers: Vec<ProviderConfig>,
    active_profile: Option<String>,
}

fn parse_providers(
    raw: &BTreeMap<String, String>,
    selected_profile: Option<&str>,
) -> Result<ProviderSelection, String> {
    let has_profile_config = raw.contains_key("profiles")
        || selected_profile.is_some()
        || !collect_profile_ids(raw).is_empty();
    if has_profile_config {
        return parse_profiled_providers(raw, selected_profile).map(
            |(active_providers, all_declared_providers, active_profile)| ProviderSelection {
                active_providers,
                all_declared_providers,
                active_profile: Some(active_profile),
            },
        );
    }
    if raw.contains_key("providers") {
        return parse_namespaced_providers(raw).map(|providers| ProviderSelection {
            all_declared_providers: providers.clone(),
            active_providers: providers,
            active_profile: None,
        });
    }
    if !collect_namespaced_provider_ids(raw).is_empty() {
        return Err(
            "Found namespaced provider keys but no `providers` order list. Add `providers \"id1,id2\"` or keep using legacy keys like provider_1_type."
                .to_owned(),
        );
    }
    parse_indexed_providers(raw).map(|providers| ProviderSelection {
        all_declared_providers: providers.clone(),
        active_providers: providers,
        active_profile: None,
    })
}

fn parse_namespaced_providers(
    raw: &BTreeMap<String, String>,
) -> Result<Vec<ProviderConfig>, String> {
    parse_namespaced_provider_defs(raw).map(|providers| {
        providers
            .into_iter()
            .map(|(_provider_id, provider)| provider)
            .collect()
    })
}

fn parse_namespaced_provider_defs(
    raw: &BTreeMap<String, String>,
) -> Result<Vec<(String, ProviderConfig)>, String> {
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
        let provider = parse_provider_from_prefix(
            raw,
            &prefix,
            provider_id.clone(),
            &format!("provider.{provider_id}"),
        )?;
        providers.push((provider_id, provider));
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

fn collect_profile_ids(raw: &BTreeMap<String, String>) -> BTreeSet<String> {
    raw.keys()
        .filter_map(|key| {
            let rest = key.strip_prefix("profile.")?;
            let (profile_id, _field) = rest.split_once('.')?;
            (!profile_id.is_empty()).then(|| profile_id.to_owned())
        })
        .collect()
}

fn parse_provider_ids(raw: &str) -> Result<Vec<String>, String> {
    parse_id_list(raw, "providers", "provider")
}

fn parse_profile_ids(raw: &str) -> Result<Vec<String>, String> {
    parse_id_list(raw, "profiles", "profile")
}

fn parse_id_list(raw: &str, collection_name: &str, item_kind: &str) -> Result<Vec<String>, String> {
    let mut provider_ids = Vec::new();
    let mut seen = BTreeSet::new();

    for provider_id in raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !is_valid_id(provider_id) {
            return Err(format!(
                "Invalid {item_kind} id `{provider_id}` in `{collection_name}`. Use only letters, numbers, `_`, and `-`."
            ));
        }
        if !seen.insert(provider_id.to_owned()) {
            return Err(format!(
                "Duplicate {item_kind} id `{provider_id}` in `{collection_name}`."
            ));
        }
        provider_ids.push(provider_id.to_owned());
    }

    if provider_ids.is_empty() {
        return Err(format!(
            "`{collection_name}` must list at least one {item_kind} id."
        ));
    }

    Ok(provider_ids)
}

fn is_valid_id(provider_id: &str) -> bool {
    provider_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
}

fn parse_profiled_providers(
    raw: &BTreeMap<String, String>,
    selected_profile: Option<&str>,
) -> Result<(Vec<ProviderConfig>, Vec<ProviderConfig>, String), String> {
    if !raw.contains_key("providers") {
        return Err(
            "Profiles require namespaced providers. Add `providers \"id1,id2\"` with `provider.<id>.*` keys."
                .to_owned(),
        );
    }

    let provider_defs = parse_namespaced_provider_defs(raw)?;
    let provider_ids = provider_defs
        .iter()
        .map(|(provider_id, _provider)| provider_id.clone())
        .collect::<BTreeSet<_>>();
    let profile_names = parse_profile_ids(
        raw.get("profiles")
            .ok_or_else(|| "Missing `profiles` config key.".to_owned())?,
    )?;
    let listed_profile_names = profile_names.iter().cloned().collect::<BTreeSet<_>>();
    let declared_profile_names = collect_profile_ids(raw);
    let unlisted_profile_names = declared_profile_names
        .difference(&listed_profile_names)
        .cloned()
        .collect::<Vec<_>>();
    if !unlisted_profile_names.is_empty() {
        return Err(format!(
            "Found profile.* keys for ids not listed in `profiles`: {}",
            unlisted_profile_names.join(", ")
        ));
    }

    let mut profiles = BTreeMap::new();
    for profile_name in &profile_names {
        let key = format!("profile.{profile_name}.providers");
        let profile_provider_ids = parse_id_list(
            raw.get(&key)
                .ok_or_else(|| format!("Missing required config key: {key}"))?,
            &key,
            "provider",
        )?;
        let unknown_provider_ids = profile_provider_ids
            .iter()
            .filter(|provider_id| !provider_ids.contains(*provider_id))
            .cloned()
            .collect::<Vec<_>>();
        if !unknown_provider_ids.is_empty() {
            return Err(format!(
                "Profile `{profile_name}` references undeclared providers: {}",
                unknown_provider_ids.join(", ")
            ));
        }
        profiles.insert(profile_name.clone(), profile_provider_ids);
    }

    let active_profile = selected_profile
        .map(str::to_owned)
        .unwrap_or_else(|| profile_names[0].clone());
    if !listed_profile_names.contains(&active_profile) {
        return Err(format!(
            "Selected profile `{active_profile}` is not listed in `profiles`."
        ));
    }

    let provider_map = provider_defs.into_iter().collect::<BTreeMap<_, _>>();
    let all_declared_providers = provider_map.values().cloned().collect::<Vec<_>>();
    let providers = profiles[&active_profile]
        .iter()
        .map(|provider_id| provider_map.get(provider_id).cloned())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            format!(
                "Internal error: selected profile `{active_profile}` could not be fully resolved"
            )
        })?;

    Ok((providers, all_declared_providers, active_profile))
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
        assert!(parsed.active_profile.is_none());
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

    #[test]
    fn resolves_provider_subset_from_selected_profile() {
        let raw = BTreeMap::from([
            ("profile".to_owned(), "task".to_owned()),
            ("profiles".to_owned(), "default,task".to_owned()),
            (
                "providers".to_owned(),
                "shell,ipython,task_snippets".to_owned(),
            ),
            ("provider.shell.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.shell.path".to_owned(),
                "~/.bash_history".to_owned(),
            ),
            ("provider.ipython.type".to_owned(), "ipython".to_owned()),
            (
                "provider.ipython.path".to_owned(),
                "~/.ipython/profile_default/history.sqlite".to_owned(),
            ),
            (
                "provider.task_snippets.type".to_owned(),
                "command_json".to_owned(),
            ),
            (
                "provider.task_snippets.command".to_owned(),
                "python3".to_owned(),
            ),
            (
                "profile.default.providers".to_owned(),
                "shell,ipython".to_owned(),
            ),
            (
                "profile.task.providers".to_owned(),
                "shell,task_snippets".to_owned(),
            ),
        ]);

        let parsed = parse_config(raw).expect("config should parse");
        assert_eq!(parsed.active_profile.as_deref(), Some("task"));
        assert_eq!(parsed.providers.len(), 2);
        assert_eq!(parsed.providers[0].name, "shell");
        assert_eq!(parsed.providers[1].name, "task_snippets");
        assert!(parsed.requires_full_hd_access);
        assert!(parsed.requires_run_commands);
    }

    #[test]
    fn defaults_to_first_profile_when_selected_profile_is_missing() {
        let raw = BTreeMap::from([
            ("profiles".to_owned(), "default,task".to_owned()),
            ("providers".to_owned(), "shell,copyq".to_owned()),
            ("provider.shell.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.shell.path".to_owned(),
                "~/.bash_history".to_owned(),
            ),
            ("provider.copyq.type".to_owned(), "command_json".to_owned()),
            ("provider.copyq.command".to_owned(), "python3".to_owned()),
            ("profile.default.providers".to_owned(), "shell".to_owned()),
            ("profile.task.providers".to_owned(), "copyq".to_owned()),
        ]);

        let parsed = parse_config(raw).expect("config should parse");
        assert_eq!(parsed.active_profile.as_deref(), Some("default"));
        assert_eq!(parsed.providers.len(), 1);
        assert_eq!(parsed.providers[0].name, "shell");
        assert!(parsed.requires_full_hd_access);
        assert!(parsed.requires_run_commands);
    }

    #[test]
    fn errors_when_selected_profile_is_not_declared() {
        let raw = BTreeMap::from([
            ("profile".to_owned(), "task".to_owned()),
            ("profiles".to_owned(), "default".to_owned()),
            ("providers".to_owned(), "shell".to_owned()),
            ("provider.shell.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.shell.path".to_owned(),
                "~/.bash_history".to_owned(),
            ),
            ("profile.default.providers".to_owned(), "shell".to_owned()),
        ]);

        let error = parse_config(raw).expect_err("config should fail");
        assert!(error.contains("Selected profile `task`"));
    }

    #[test]
    fn errors_when_profile_references_undeclared_provider() {
        let raw = BTreeMap::from([
            ("profiles".to_owned(), "default".to_owned()),
            ("providers".to_owned(), "shell".to_owned()),
            ("provider.shell.type".to_owned(), "file_lines".to_owned()),
            (
                "provider.shell.path".to_owned(),
                "~/.bash_history".to_owned(),
            ),
            (
                "profile.default.providers".to_owned(),
                "shell,missing".to_owned(),
            ),
        ]);

        let error = parse_config(raw).expect_err("config should fail");
        assert!(error.contains("undeclared providers"));
        assert!(error.contains("missing"));
    }

    #[test]
    fn errors_when_profiles_are_used_without_namespaced_providers() {
        let raw = BTreeMap::from([
            ("profile".to_owned(), "default".to_owned()),
            ("profiles".to_owned(), "default".to_owned()),
            ("profile.default.providers".to_owned(), "shell".to_owned()),
            ("provider_1_type".to_owned(), "file_lines".to_owned()),
            ("provider_1_path".to_owned(), "~/.bash_history".to_owned()),
        ]);

        let error = parse_config(raw).expect_err("config should fail");
        assert!(error.contains("Profiles require namespaced providers"));
    }
}
