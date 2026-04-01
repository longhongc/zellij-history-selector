use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::model::{MatchResult, ProviderKind, ProviderLoadState, ProviderState};

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_DIM: &str = "\x1b[2m";
const ANSI_ITALIC: &str = "\x1b[3m";
const ANSI_BLUE: &str = "\x1b[34m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_BRIGHT_BLACK: &str = "\x1b[90m";

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
    show_footer: bool,
    target_label: &str,
) -> String {
    let width = cols.max(1);
    let height = rows.max(1);
    let footer_rows = usize::from(show_footer);
    let content_height = height.saturating_sub(footer_rows);
    let provider = providers.get(current_provider);
    let provider_name = provider
        .map(|provider| provider.config.name.as_str())
        .unwrap_or("none");
    let provider_state = provider
        .map(|provider| format_provider_state(&provider.load_state))
        .unwrap_or_else(|| "missing".to_owned());

    let header_rows = content_height.min(3);
    let spacer_rows = usize::from(content_height > header_rows);
    let preview_header_rows = usize::from(content_height > header_rows + spacer_rows);
    let max_preview_body =
        content_height.saturating_sub(header_rows + spacer_rows + preview_header_rows);
    let preview_body_height = if max_preview_body == 0 {
        0
    } else {
        preview_lines.min(max_preview_body).min(content_height / 3)
    };
    let preview_header_rows = usize::from(preview_body_height > 0);
    let spacer_rows = usize::from(preview_body_height > 0);
    let list_height = content_height
        .saturating_sub(header_rows + spacer_rows + preview_header_rows + preview_body_height);

    let mut lines = Vec::with_capacity(content_height);
    lines.push(pad_right(
        &format!(
            "Provider: {} [{}]  Target: {}",
            provider_name, provider_state, target_label
        ),
        width,
    ));
    if content_height > 1 {
        lines.push(pad_right(&format!("> {}", query), width));
    }
    if content_height > 2 {
        lines.push(pad_right(
            &match_count_line(provider, matches, width),
            width,
        ));
    }

    let rendered_matches =
        render_match_lines(provider, matches, selected_match, width, list_height);
    lines.extend(rendered_matches);

    if preview_body_height > 0 {
        lines.push(" ".repeat(width));
        lines.push(pad_right(&section_label("Preview", width), width));
        lines.extend(render_preview_lines(
            provider,
            matches,
            selected_match,
            width,
            preview_body_height,
        ));
    }

    if show_footer {
        let (footer, footer_visible_width) = footer_hint_line(status);
        while lines.len() + 1 < height {
            lines.push(" ".repeat(width));
        }
        lines.push(pad_right_ansi(&footer, footer_visible_width, width));
        lines.truncate(height);
    } else {
        lines.truncate(content_height);
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
        }
        Some(ProviderLoadState::Error(error)) => {
            lines.push(pad_right(&format!("Load error: {}", error), width));
        }
        Some(ProviderLoadState::Ready(entries)) => {
            if matches.is_empty() {
                lines.push(pad_right("No matches", width));
            } else {
                let start = selected_match.saturating_sub(list_height / 2);
                for (offset, match_result) in
                    matches.iter().skip(start).take(list_height).enumerate()
                {
                    let selected = start + offset == selected_match;
                    let text = entries
                        .get(match_result.entry_index)
                        .map(|entry| first_line(&entry.text))
                        .unwrap_or("");
                    let clipped = truncate_to_width(text, width.saturating_sub(4));
                    let styled = maybe_style_python(provider, &clipped, selected);
                    let prefix = if selected { "❯ " } else { "  " };
                    let visible = 2 + visible_width(&clipped);
                    lines.push(pad_right_ansi(&format!("{prefix}{styled}"), visible, width));
                }
            }
        }
        _ => {
            lines.push(pad_right("Waiting for provider...", width));
        }
    }

    lines.truncate(list_height);
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
                for line in preview
                    .lines()
                    .take(preview_height.saturating_sub(lines.len()))
                {
                    let clipped = truncate_to_width(line, width.saturating_sub(2));
                    let styled = maybe_style_python(provider, &clipped, false);
                    let visible = 2 + visible_width(&clipped);
                    lines.push(pad_right_ansi(&format!("  {styled}"), visible, width));
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

fn pad_right_ansi(text: &str, visible_len: usize, width: usize) -> String {
    let padding = width.saturating_sub(visible_len);
    format!("{}{}", text, " ".repeat(padding))
}

fn visible_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn section_label(label: &str, width: usize) -> String {
    let content = format!("{} ", label);
    let remaining = width.saturating_sub(visible_width(&content));
    format!("{}{}", content, "─".repeat(remaining))
}

fn match_count_line(
    provider: Option<&ProviderState>,
    matches: &[MatchResult],
    width: usize,
) -> String {
    let total = match provider.map(|provider| &provider.load_state) {
        Some(ProviderLoadState::Ready(entries)) => entries.len(),
        _ => 0,
    };
    let matched = matches.len();
    let prefix = format!("{matched}/{total} ");
    let divider = "─".repeat(width.saturating_sub(visible_width(&prefix)));
    format!("{prefix}{divider}")
}

fn maybe_style_python(provider: Option<&ProviderState>, text: &str, selected: bool) -> String {
    let styled = if provider_looks_python(provider) {
        highlight_python(text)
    } else {
        text.to_owned()
    };
    if selected {
        format!("{ANSI_BOLD}{styled}{ANSI_RESET}")
    } else {
        styled
    }
}

fn footer_hint_line(status: Option<&str>) -> (String, usize) {
    let mut rendered = String::new();
    let mut total_visible_width = 0usize;

    for segment in [
        styled_segment("ENTER", ANSI_BOLD),
        styled_segment(" select", ANSI_DIM),
        plain_segment("·"),
        styled_segment("UP/DOWN", ANSI_BOLD),
        styled_segment(" move", ANSI_DIM),
        plain_segment("·"),
        styled_segment("TAB", ANSI_BOLD),
        styled_segment(" source", ANSI_DIM),
        plain_segment("·"),
        styled_segment("ESC", ANSI_BOLD),
        styled_segment(" close", ANSI_DIM),
        plain_segment("·"),
        styled_segment("?", ANSI_BOLD),
        styled_segment(" help", ANSI_DIM),
    ] {
        rendered.push_str(&segment.rendered);
        total_visible_width += segment.visible_width;
    }

    if let Some(status) = status.filter(|status| *status != "ready") {
        let clipped = truncate_to_width(status, 28);
        rendered.push_str(ANSI_DIM);
        rendered.push('·');
        rendered.push_str(ANSI_RESET);
        rendered.push_str("\x1b[31m");
        rendered.push_str("status:");
        rendered.push_str(ANSI_RESET);
        rendered.push_str(ANSI_DIM);
        rendered.push(' ');
        rendered.push_str(&clipped);
        rendered.push_str(ANSI_RESET);
        total_visible_width += visible_width("·status: ") + visible_width(&clipped);
    }

    (rendered, total_visible_width)
}

struct FooterSegment {
    rendered: String,
    visible_width: usize,
}

fn plain_segment(text: &'static str) -> FooterSegment {
    FooterSegment {
        rendered: text.to_owned(),
        visible_width: visible_width(text),
    }
}

fn styled_segment(text: &'static str, style: &'static str) -> FooterSegment {
    FooterSegment {
        rendered: format!("{style}{text}{ANSI_RESET}"),
        visible_width: visible_width(text),
    }
}

fn provider_looks_python(provider: Option<&ProviderState>) -> bool {
    matches!(
        provider.map(|provider| &provider.config.kind),
        Some(ProviderKind::IPython(_))
    )
}

fn highlight_python(text: &str) -> String {
    let keywords = [
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
        "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True",
        "try", "while", "with", "yield",
    ];

    let mut rendered = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '#' {
            let rest: String = chars[i..].iter().collect();
            rendered.push_str(ANSI_BRIGHT_BLACK);
            rendered.push_str(ANSI_ITALIC);
            rendered.push_str(&rest);
            rendered.push_str(ANSI_RESET);
            break;
        } else if c == '\'' || c == '"' {
            let quote = c;
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let token: String = chars[start..i.min(chars.len())].iter().collect();
            rendered.push_str(ANSI_GREEN);
            rendered.push_str(&token);
            rendered.push_str(ANSI_RESET);
            continue;
        } else if c == '%' && (i == 0 || chars[i - 1].is_whitespace()) {
            let start = i;
            i += 1;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            rendered.push_str(ANSI_YELLOW);
            rendered.push_str(&token);
            rendered.push_str(ANSI_RESET);
            continue;
        } else if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            rendered.push_str(ANSI_CYAN);
            rendered.push_str(&token);
            rendered.push_str(ANSI_RESET);
            continue;
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            if keywords.contains(&token.as_str()) {
                rendered.push_str(ANSI_BLUE);
                rendered.push_str(ANSI_BOLD);
                rendered.push_str(&token);
                rendered.push_str(ANSI_RESET);
            } else {
                rendered.push_str(&token);
            }
            continue;
        } else {
            rendered.push(c);
            i += 1;
        }
    }
    rendered
}
