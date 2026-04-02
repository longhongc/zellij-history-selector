mod config;
mod fuzzy;
mod model;
mod provider;
mod ui;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use config::parse_config;
use fuzzy::filter_entries;
use model::{AppConfig, DefaultMode, MatchResult, ProviderLoadState, ProviderState};
use provider::{
    build_command_invocation, load_file_provider, parse_command_output, provider_requires_full_hd,
    provider_requires_run_commands, CommandInvocation,
};
use zellij_tile::prelude::*;

const PREFERRED_FLOATING_HEIGHT: &str = "84%";
const PLUGIN_ID_HINT: &str = "zellij-history-selector";

#[derive(Default)]
struct State {
    app_config: Option<AppConfig>,
    providers: Vec<ProviderState>,
    current_provider: usize,
    query: String,
    filtered: Vec<MatchResult>,
    selected_match: usize,
    startup_error: Option<String>,
    status_message: Option<String>,
    permission_granted: bool,
    host_ready: bool,
    needs_host_root: bool,
    target_pane: Option<PaneId>,
    pane_manifest: Option<PaneManifest>,
    pending_requests: HashMap<String, usize>,
    next_request_id: usize,
    initialized: bool,
    help_visible: bool,
    self_pane: Option<PaneId>,
    floating_height_adjusted: bool,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        subscribe(&[
            EventType::Key,
            EventType::ListClients,
            EventType::PaneUpdate,
            EventType::SystemClipboardFailure,
            EventType::PermissionRequestResult,
            EventType::RunCommandResult,
            EventType::HostFolderChanged,
            EventType::FailedToChangeHostFolder,
        ]);

        match parse_config(configuration) {
            Ok(app_config) => {
                self.needs_host_root = app_config.providers.iter().any(provider_requires_full_hd);
                self.providers = app_config
                    .providers
                    .iter()
                    .cloned()
                    .map(ProviderState::new)
                    .collect();
                self.app_config = Some(app_config);
                self.help_visible = true;
                self.request_permissions();
            }
            Err(error) => {
                self.startup_error = Some(error);
            }
        }
    }

    fn update(&mut self, event: Event) -> bool {
        let should_render = match event {
            Event::PermissionRequestResult(status) => {
                self.handle_permission_result(status);
                true
            }
            Event::ListClients(clients) => {
                if let Some(client) = clients.into_iter().find(|client| client.is_current_client) {
                    self.target_pane = Some(client.pane_id);
                    self.try_start_loading();
                }
                true
            }
            Event::PaneUpdate(pane_manifest) => {
                if self.target_pane.is_none() {
                    self.target_pane = preferred_target_from_manifest(&pane_manifest);
                }
                self.pane_manifest = Some(pane_manifest);
                self.try_start_loading();
                true
            }
            Event::HostFolderChanged(_path) => {
                self.host_ready = true;
                self.try_start_loading();
                true
            }
            Event::FailedToChangeHostFolder(error) => {
                self.status_message = Some(
                    error.unwrap_or_else(|| "Failed to mount /host at filesystem root".to_owned()),
                );
                true
            }
            Event::SystemClipboardFailure => {
                self.status_message = Some(
                    "Failed to copy to clipboard. Check your Zellij clipboard configuration."
                        .to_owned(),
                );
                true
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                self.handle_run_command_result(exit_code, stdout, stderr, context);
                true
            }
            Event::Key(key) => self.handle_key(key),
            _ => false,
        };

        if should_render {
            self.maybe_resize_self_floating_pane();
        }

        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if let Some(error) = self.startup_error.as_deref() {
            print!("{}", error);
            return;
        }

        let target_label = self
            .target_pane
            .as_ref()
            .map(|pane_id| format!("{pane_id:?}"))
            .unwrap_or_else(|| "capturing...".to_owned());
        let status = if !self.permission_granted {
            Some("Approve the Zellij permission prompt for this plugin.")
        } else {
            self.status_message.as_deref()
        };
        let preview_lines = self
            .app_config
            .as_ref()
            .map(|config| config.preview_lines)
            .unwrap_or(10);
        let show_footer = self.help_visible || status.is_some();
        let screen = ui::render_screen(
            rows,
            cols,
            &self.providers,
            self.current_provider,
            &self.query,
            &self.filtered,
            self.selected_match,
            preview_lines,
            status,
            show_footer,
            &target_label,
        );
        print!("{}", screen);
    }
}

impl State {
    fn maybe_resize_self_floating_pane(&mut self) {
        if self.floating_height_adjusted || !self.permission_granted {
            return;
        }

        let Some(self_pane) = self.detect_self_pane() else {
            return;
        };
        let Some(coordinates) = FloatingPaneCoordinates::new(
            None,
            None,
            None,
            Some(PREFERRED_FLOATING_HEIGHT.to_owned()),
            None,
            None,
        ) else {
            return;
        };

        change_floating_panes_coordinates(vec![(self_pane, coordinates)]);
        self.self_pane = Some(self_pane);
        self.floating_height_adjusted = true;
    }

    fn detect_self_pane(&self) -> Option<PaneId> {
        if let Some(self_pane) = self.self_pane {
            return Some(self_pane);
        }

        if let Ok((_tab_index, pane_id)) = get_focused_pane_info() {
            if pane_id_is_plugin(pane_id) {
                return Some(pane_id);
            }
        }

        self.pane_manifest
            .as_ref()
            .and_then(preferred_plugin_from_manifest)
    }

    fn request_permissions(&mut self) {
        let Some(app_config) = self.app_config.as_ref() else {
            return;
        };

        let mut permissions = BTreeSet::from([
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::WriteToStdin,
        ]);
        if matches!(app_config.default_mode, DefaultMode::Copy) {
            permissions.insert(PermissionType::WriteToClipboard);
        }
        if app_config
            .providers
            .iter()
            .any(provider_requires_run_commands)
        {
            permissions.insert(PermissionType::RunCommands);
        }
        if self.needs_host_root {
            permissions.insert(PermissionType::FullHdAccess);
        }
        request_permission(&permissions.into_iter().collect::<Vec<_>>());
    }

    fn handle_permission_result(&mut self, status: PermissionStatus) {
        match status {
            PermissionStatus::Granted => {
                self.permission_granted = true;
                list_clients();
                if self.needs_host_root {
                    change_host_folder(PathBuf::from("/"));
                } else {
                    self.host_ready = true;
                    self.try_start_loading();
                }
            }
            PermissionStatus::Denied => {
                self.status_message = Some(
                    "Permission request denied. The plugin needs application state access, pane writes, and provider-specific filesystem/command permissions."
                        .to_owned(),
                );
            }
        }
    }

    fn try_start_loading(&mut self) {
        if self.initialized {
            return;
        }
        if !self.permission_granted {
            return;
        }
        if self.needs_host_root && !self.host_ready {
            return;
        }
        self.initialized = true;
        self.reload_current_provider();
    }

    fn reload_current_provider(&mut self) {
        let Some(provider_state) = self.providers.get_mut(self.current_provider) else {
            return;
        };
        self.status_message = None;
        self.selected_match = 0;
        match load_current_provider(
            provider_state,
            self.current_provider,
            &mut self.next_request_id,
        ) {
            LoadOutcome::Ready(entries) => {
                provider_state.load_state = ProviderLoadState::Ready(entries);
                self.recompute_matches();
            }
            LoadOutcome::Pending {
                request_id,
                invocation,
            } => {
                provider_state.load_state = ProviderLoadState::Loading;
                self.pending_requests
                    .insert(request_id.clone(), self.current_provider);
                self.run_command_invocation(&request_id, invocation);
                self.recompute_matches();
            }
            LoadOutcome::Error(error) => {
                provider_state.load_state = ProviderLoadState::Error(error);
                self.recompute_matches();
            }
        }
    }

    fn run_command_invocation(&mut self, request_id: &str, invocation: CommandInvocation) {
        let argv_refs = invocation
            .argv
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let context = BTreeMap::from([("request_id".to_owned(), request_id.to_owned())]);
        if let Some(cwd) = invocation.cwd {
            run_command_with_env_variables_and_cwd(&argv_refs, invocation.env, cwd, context);
        } else {
            run_command(&argv_refs, context);
        }
    }

    fn handle_run_command_result(
        &mut self,
        exit_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        context: BTreeMap<String, String>,
    ) {
        let Some(request_id) = context.get("request_id") else {
            return;
        };
        let Some(provider_index) = self.pending_requests.remove(request_id) else {
            return;
        };
        let Some(provider_state) = self.providers.get_mut(provider_index) else {
            return;
        };

        match parse_command_output(&provider_state.config, exit_code, &stdout, &stderr) {
            Ok(entries) => provider_state.load_state = ProviderLoadState::Ready(entries),
            Err(error) => provider_state.load_state = ProviderLoadState::Error(error),
        }
        self.recompute_matches();
    }

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        if self.startup_error.is_some() {
            return false;
        }

        if is_ctrl_char(&key, 'c') {
            close_self();
            return false;
        }

        match key.bare_key {
            BareKey::Esc => {
                close_self();
                false
            }
            BareKey::Enter => {
                self.hide_help();
                self.select_current_entry();
                true
            }
            BareKey::Up => {
                self.hide_help();
                self.move_selection(-1);
                true
            }
            BareKey::Down => {
                self.hide_help();
                self.move_selection(1);
                true
            }
            BareKey::PageUp => {
                self.hide_help();
                self.move_selection(-10);
                true
            }
            BareKey::PageDown => {
                self.hide_help();
                self.move_selection(10);
                true
            }
            BareKey::Home => {
                self.hide_help();
                self.selected_match = 0;
                true
            }
            BareKey::End => {
                self.hide_help();
                self.selected_match = self.filtered.len().saturating_sub(1);
                true
            }
            BareKey::Backspace => {
                self.hide_help();
                self.query.pop();
                self.recompute_matches();
                true
            }
            BareKey::Delete => {
                self.hide_help();
                self.query.clear();
                self.recompute_matches();
                true
            }
            BareKey::Tab => {
                self.hide_help();
                if key.key_modifiers.contains(&KeyModifier::Shift) {
                    self.cycle_provider(-1);
                } else {
                    self.cycle_provider(1);
                }
                true
            }
            BareKey::Char(character) => {
                if !key.key_modifiers.contains(&KeyModifier::Ctrl)
                    && !key.key_modifiers.contains(&KeyModifier::Alt)
                    && character == '?'
                {
                    self.help_visible = !self.help_visible;
                    return true;
                }
                if is_ctrl_char(&key, 'j') {
                    self.hide_help();
                    self.move_selection(1);
                    return true;
                }
                if is_ctrl_char(&key, 'k') {
                    self.hide_help();
                    self.move_selection(-1);
                    return true;
                }
                if is_ctrl_char(&key, 'r') {
                    self.hide_help();
                    self.reload_current_provider();
                    return true;
                }
                if !key.key_modifiers.contains(&KeyModifier::Ctrl)
                    && !key.key_modifiers.contains(&KeyModifier::Alt)
                {
                    self.hide_help();
                    self.query.push(character);
                    self.recompute_matches();
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn cycle_provider(&mut self, delta: isize) {
        if self.providers.is_empty() {
            return;
        }
        let len = self.providers.len() as isize;
        let next = (self.current_provider as isize + delta).rem_euclid(len) as usize;
        self.current_provider = next;
        self.query.clear();
        self.selected_match = 0;
        if matches!(self.providers[next].load_state, ProviderLoadState::Unloaded) {
            self.reload_current_provider();
        } else {
            self.recompute_matches();
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            self.selected_match = 0;
            return;
        }
        let next =
            (self.selected_match as isize + delta).clamp(0, self.filtered.len() as isize - 1);
        self.selected_match = next as usize;
    }

    fn recompute_matches(&mut self) {
        let Some(app_config) = self.app_config.as_ref() else {
            self.filtered.clear();
            self.selected_match = 0;
            return;
        };
        let Some(provider_state) = self.providers.get(self.current_provider) else {
            self.filtered.clear();
            self.selected_match = 0;
            return;
        };

        self.filtered = match &provider_state.load_state {
            ProviderLoadState::Ready(entries) => filter_entries(
                entries,
                &self.query,
                app_config.case_sensitive,
                app_config.max_results,
            ),
            _ => Vec::new(),
        };
        if self.filtered.is_empty() {
            self.selected_match = 0;
        } else if self.selected_match >= self.filtered.len() {
            self.selected_match = self.filtered.len() - 1;
        }
    }

    fn select_current_entry(&mut self) {
        let Some(app_config) = self.app_config.as_ref() else {
            return;
        };
        let Some(provider_state) = self.providers.get(self.current_provider) else {
            return;
        };
        let ProviderLoadState::Ready(entries) = &provider_state.load_state else {
            return;
        };
        let Some(match_result) = self.filtered.get(self.selected_match) else {
            return;
        };
        let Some(entry) = entries.get(match_result.entry_index) else {
            return;
        };

        if matches!(app_config.default_mode, DefaultMode::Copy) {
            copy_to_clipboard(entry.text.clone());
            close_self();
            return;
        }

        let Some(target_pane) = self.target_pane.or_else(|| {
            self.pane_manifest
                .as_ref()
                .and_then(preferred_target_from_manifest)
        }) else {
            self.status_message = Some("Could not resolve a target pane for insertion.".to_owned());
            return;
        };
        self.target_pane = Some(target_pane);
        if !self.target_pane_exists(target_pane) {
            self.status_message = Some(
                "The original pane no longer exists. Retry after it returns, or press Esc to close."
                    .to_owned(),
            );
            return;
        }

        let mut text = entry.text.clone();
        if matches!(app_config.default_mode, DefaultMode::Execute) {
            text.push('\n');
        }
        write_chars_to_pane_id(&text, target_pane);
        close_self();
    }

    fn target_pane_exists(&self, target_pane: PaneId) -> bool {
        let Some(pane_manifest) = self.pane_manifest.as_ref() else {
            return true;
        };
        pane_manifest.panes.values().flatten().any(|pane_info| {
            pane_info.id == pane_id_number(target_pane)
                && pane_info.is_plugin == pane_id_is_plugin(target_pane)
        })
    }

    fn hide_help(&mut self) {
        self.help_visible = false;
    }
}

enum LoadOutcome {
    Ready(Vec<model::HistoryEntry>),
    Pending {
        request_id: String,
        invocation: CommandInvocation,
    },
    Error(String),
}

fn load_current_provider(
    provider_state: &ProviderState,
    provider_index: usize,
    next_request_id: &mut usize,
) -> LoadOutcome {
    match &provider_state.config.kind {
        model::ProviderKind::FileLines(_) => match load_file_provider(&provider_state.config) {
            Ok(entries) => LoadOutcome::Ready(entries),
            Err(error) => LoadOutcome::Error(error),
        },
        _ => match build_command_invocation(&provider_state.config) {
            Ok(mut invocation) => {
                let request_id = format!("provider-{provider_index}-{}", *next_request_id);
                *next_request_id += 1;
                invocation.env.insert(
                    "ZHS_PROVIDER".to_owned(),
                    provider_state.config.name.clone(),
                );
                LoadOutcome::Pending {
                    request_id,
                    invocation,
                }
            }
            Err(error) => LoadOutcome::Error(error),
        },
    }
}

fn is_ctrl_char(key: &KeyWithModifier, character: char) -> bool {
    key.bare_key == BareKey::Char(character) && key.key_modifiers.contains(&KeyModifier::Ctrl)
}

fn pane_id_number(pane_id: PaneId) -> u32 {
    match pane_id {
        PaneId::Terminal(id) => id,
        PaneId::Plugin(id) => id,
    }
}

fn pane_id_is_plugin(pane_id: PaneId) -> bool {
    matches!(pane_id, PaneId::Plugin(_))
}

fn preferred_target_from_manifest(pane_manifest: &PaneManifest) -> Option<PaneId> {
    pane_manifest
        .panes
        .values()
        .flatten()
        .find(|pane_info| pane_info.is_selectable && !pane_info.is_plugin && pane_info.is_focused)
        .or_else(|| {
            pane_manifest
                .panes
                .values()
                .flatten()
                .find(|pane_info| pane_info.is_selectable && !pane_info.is_plugin)
        })
        .map(|pane_info| {
            if pane_info.is_plugin {
                PaneId::Plugin(pane_info.id)
            } else {
                PaneId::Terminal(pane_info.id)
            }
        })
}

fn preferred_plugin_from_manifest(pane_manifest: &PaneManifest) -> Option<PaneId> {
    pane_manifest
        .panes
        .values()
        .flatten()
        .find(|pane_info| pane_info.is_focused && is_this_plugin_pane(pane_info))
        .or_else(|| {
            pane_manifest
                .panes
                .values()
                .flatten()
                .find(|pane_info| is_this_plugin_pane(pane_info))
        })
        .map(|pane_info| PaneId::Plugin(pane_info.id))
}

fn is_this_plugin_pane(pane_info: &PaneInfo) -> bool {
    pane_info.is_plugin
        && pane_info.is_selectable
        && !pane_info.is_suppressed
        && (pane_info.title.contains(PLUGIN_ID_HINT)
            || pane_info
                .plugin_url
                .as_deref()
                .is_some_and(|plugin_url| plugin_url.contains(PLUGIN_ID_HINT)))
}
