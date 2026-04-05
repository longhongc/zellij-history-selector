use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefaultMode {
    Insert,
    Execute,
    Copy,
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    #[allow(dead_code)]
    pub active_profile: Option<String>,
    pub default_mode: DefaultMode,
    pub max_results: usize,
    pub preview_lines: usize,
    pub case_sensitive: bool,
    pub requires_run_commands: bool,
    pub requires_full_hd_access: bool,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub name: String,
    pub kind: ProviderKind,
}

#[derive(Clone, Debug)]
pub enum ProviderKind {
    FileLines(FileLinesConfig),
    SqliteQuery(SqliteQueryConfig),
    Command(CommandConfig),
    IPython(IPythonConfig),
}

#[derive(Clone, Debug)]
pub struct FileLinesConfig {
    pub path: String,
    pub reverse: bool,
    pub dedupe: bool,
    pub limit: usize,
}

#[derive(Clone, Debug)]
pub struct SqliteQueryConfig {
    pub path: String,
    pub query: String,
    pub text_column: usize,
    pub preview_column: Option<usize>,
    #[allow(dead_code)]
    pub timestamp_column: Option<usize>,
    pub limit: usize,
    pub dedupe: bool,
}

#[derive(Clone, Debug)]
pub enum CommandOutputMode {
    Lines,
    Json,
}

#[derive(Clone, Debug)]
pub struct CommandConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: BTreeMap<String, String>,
    pub output_mode: CommandOutputMode,
    pub limit: usize,
    pub dedupe: bool,
}

#[derive(Clone, Debug)]
pub struct IPythonConfig {
    pub path: String,
    pub query_override: Option<String>,
    pub limit: usize,
    pub dedupe: bool,
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub text: String,
    pub preview: Option<String>,
    pub score_hint: i64,
}

#[derive(Clone, Debug)]
pub enum ProviderLoadState {
    Unloaded,
    Loading,
    Ready(Vec<HistoryEntry>),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct ProviderState {
    pub config: ProviderConfig,
    pub load_state: ProviderLoadState,
}

impl ProviderState {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            load_state: ProviderLoadState::Unloaded,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MatchResult {
    pub entry_index: usize,
    pub score: i64,
}
