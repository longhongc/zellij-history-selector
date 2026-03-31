use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::model::{MatchResult, ProviderLoadState, ProviderState};

pub fn render_screen(
    rows: usize,
    cols: usize,
    providers: &[ProviderState],
    current_provider: usize,
    query: &str,
    matches: &[MatchResult],
    selected_match: usize,
    preview_lines: usize,
    status: Option<&str>,
    target_label: &str,
) -> String {
    let width = cols.max(20);
    let height = rows.max(8);
    let provider = providers.get(current_provider);
    let title = "zellij-history-selector";
    let provider_name = provider.map(|provider| provider.config.name.as_str()).unwrap_or("none");
    let provider_state = provider
        .map(|provider| format_provider_state(&provider.load_state))
        .unwrap_or_else(|| "missing".to_owned());

    let fixed_rows = 6usize;
    let preview_height = preview_lines.min(height.saturating_sub(fixed_rows)).max(3);
    let list_height = height.saturating_sub(fixed_rows + preview_height);

    let mut lines = Vec::with_capacity(height);
    lines.push(pad_right(title, width));
    lines.push(pad_right(
        &format!("Provider: {} [{}]  Target: {}", provider_name, provider_state, target_label),
        width,
    ));
    lines.push(pad_right(
        &format!("Status: {}", status.unwrap_or("ready")),
        width,
    ));
    lines.push(pad_right(&format!("Search: {}", query), width));

    let rendered_matches = render_match_lines(provider, matches, selected_match, width, list_height);
    lines.extend(rendered_matches);

    lines.push(pad_right("", width));
    lines.push(pad_right("Preview:", width));
    lines.extend(render_preview_lines(
        provider,
        matches,
        selected_match,
        width,
        preview_height.saturating_sub(1),
    ));

    if lines.len() < height {
        let footer =
            "Enter select  Esc cancel  Ctrl+C cancel  Up/Down move  Tab switch provider  Ctrl+R reload";
        while lines.len() + 1 < height {
            lines.push(pad_right("", width));
        }
        lines.push(pad_right(footer, width));
    }

    lines.truncate(height);
    while lines.len() < height {
        lines.push(" ".repeat(width));
    }

    lines.join("\n")
}

fn render_match_lines(
    provider: Option<&ProviderState>,
    matches: &[MatchResult],
    selected_match: usize,
    width: usize,
    list_height: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    match provider.map(|provider| &provider.load_state) {
        Some(ProviderLoadState::Loading) => {
            lines.push(pad_right("Loading entries...", width));
        },
        Some(ProviderLoadState::Error(error)) => {
            lines.push(pad_right(&format!("Load error: {}", error), width));
        },
        Some(ProviderLoadState::Ready(entries)) => {
            if matches.is_empty() {
                lines.push(pad_right("No matches", width));
            } else {
                let start = selected_match.saturating_sub(list_height.saturating_sub(1));
                for (offset, match_result) in matches.iter().skip(start).take(list_height).enumerate() {
                    let selected = start + offset == selected_match;
                    let prefix = if selected { ">" } else { " " };
                    let text = entries
                        .get(match_result.entry_index)
                        .map(|entry| first_line(&entry.text))
                        .unwrap_or("");
                    lines.push(pad_right(&format!("{prefix} {}", truncate_to_width(text, width.saturating_sub(2))), width));
                }
            }
        },
        _ => {
            lines.push(pad_right("Waiting for provider...", width));
        },
    }

    while lines.len() < list_height {
        lines.push(" ".repeat(width));
    }
    lines
}

fn render_preview_lines(
    provider: Option<&ProviderState>,
    matches: &[MatchResult],
    selected_match: usize,
    width: usize,
    preview_height: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(ProviderState {
        load_state: ProviderLoadState::Ready(entries),
        ..
    }) = provider
    {
        if let Some(match_result) = matches.get(selected_match) {
            if let Some(entry) = entries.get(match_result.entry_index) {
                let preview = entry.preview.as_deref().unwrap_or(&entry.text);
                lines.push(pad_right(
                    &format!("Source: {}  Id: {}", entry.provider_name, entry.id),
                    width,
                ));
                if let Some(timestamp) = entry.timestamp.as_deref() {
                    lines.push(pad_right(&format!("Timestamp: {}", timestamp), width));
                }
                if let Some(path) = entry.metadata.get("path") {
                    if lines.len() < preview_height {
                        lines.push(pad_right(&format!("Path: {}", path), width));
                    }
                }
                for line in preview.lines().take(preview_height.saturating_sub(lines.len())) {
                    lines.push(pad_right(&truncate_to_width(line, width), width));
                }
            }
        }
    }
    while lines.len() < preview_height {
        lines.push(" ".repeat(width));
    }
    lines
}

fn format_provider_state(load_state: &ProviderLoadState) -> String {
    match load_state {
        ProviderLoadState::Unloaded => "idle".to_owned(),
        ProviderLoadState::Loading => "loading".to_owned(),
        ProviderLoadState::Ready(entries) => format!("{} entries", entries.len()),
        ProviderLoadState::Error(_) => "error".to_owned(),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text)
}

fn truncate_to_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let mut rendered = String::new();
    let mut current_width = 0usize;
    for character in text.chars() {
        let char_width = character.width().unwrap_or(0);
        if current_width + char_width > width {
            break;
        }
        rendered.push(character);
        current_width += char_width;
    }

    if UnicodeWidthStr::width(text) > width && width >= 3 {
        let mut shortened = String::new();
        let mut shortened_width = 0usize;
        for character in rendered.chars() {
            let char_width = character.width().unwrap_or(0);
            if shortened_width + char_width > width.saturating_sub(3) {
                break;
            }
            shortened.push(character);
            shortened_width += char_width;
        }
        shortened.push_str("...");
        shortened
    } else {
        rendered
    }
}

fn pad_right(text: &str, width: usize) -> String {
    let clipped = truncate_to_width(text, width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(clipped.as_str()));
    format!("{}{}", clipped, " ".repeat(padding))
}
