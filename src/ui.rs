use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::model::{
    CommandOutputMode, MatchResult, ProviderKind, ProviderLoadState, ProviderState,
};

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
    let provider_line_prefix =
        format!("Provider: {} [{}]  Target: ", provider_name, provider_state);
    let target_width = width.saturating_sub(visible_width(&provider_line_prefix));
    let target_display = truncate_from_start(target_label, target_width);
    lines.push(pad_right(
        &format!("{provider_line_prefix}{target_display}"),
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
        lines.push(pad_right(
            &section_label(preview_section_label(provider), width),
            width,
        ));
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
            lines.extend(
                wrap_plain_text(&format!("Load error: {}", error), width)
                    .into_iter()
                    .take(list_height)
                    .map(|line| pad_right(&line, width)),
            );
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
                    let styled = maybe_style_entry(provider, &clipped, selected);
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
    if let Some(ProviderState { load_state, .. }) = provider {
        match load_state {
            ProviderLoadState::Ready(entries) => {
                if let Some(match_result) = matches.get(selected_match) {
                    if let Some(entry) = entries.get(match_result.entry_index) {
                        let preview = entry.preview.as_deref().unwrap_or(&entry.text);
                        for line in preview
                            .lines()
                            .take(preview_height.saturating_sub(lines.len()))
                        {
                            let clipped = truncate_to_width(line, width.saturating_sub(2));
                            let styled = maybe_style_entry(provider, &clipped, false);
                            let visible = 2 + visible_width(&clipped);
                            lines.push(pad_right_ansi(&format!("  {styled}"), visible, width));
                        }
                    }
                }
            }
            ProviderLoadState::Error(error) => {
                lines.extend(
                    wrap_plain_text(error, width.saturating_sub(2))
                        .into_iter()
                        .take(preview_height)
                        .map(|line| pad_right(&format!("  {line}"), width)),
                );
            }
            _ => {}
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

fn truncate_from_start(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(text) <= width {
        return text.to_owned();
    }
    if width < 3 {
        return truncate_to_width(text, width);
    }

    let suffix_width = width.saturating_sub(3);
    let mut reversed = Vec::new();
    let mut current_width = 0usize;
    for character in text.chars().rev() {
        let char_width = character.width().unwrap_or(0);
        if current_width + char_width > suffix_width {
            break;
        }
        reversed.push(character);
        current_width += char_width;
    }
    reversed.reverse();

    let mut shortened = String::from("...");
    for character in reversed {
        shortened.push(character);
    }
    shortened
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

fn wrap_plain_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    let mut wrapped = Vec::new();
    for paragraph in text.lines() {
        if paragraph.trim().is_empty() {
            if !wrapped.is_empty() {
                wrapped.push(String::new());
            }
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let word_width = visible_width(word);
            if current.is_empty() {
                if word_width <= width {
                    current.push_str(word);
                } else {
                    wrapped.extend(split_long_token(word, width));
                }
                continue;
            }

            let candidate_width = visible_width(&current) + 1 + word_width;
            if candidate_width <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                wrapped.push(current);
                if word_width <= width {
                    current = word.to_owned();
                } else {
                    wrapped.extend(split_long_token(word, width));
                    current = String::new();
                }
            }
        }

        if !current.is_empty() {
            wrapped.push(current);
        }
    }

    if wrapped.is_empty() {
        wrapped.push(String::new());
    }

    wrapped
}

fn split_long_token(token: &str, width: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for character in token.chars() {
        let char_width = character.width().unwrap_or(0);
        if current_width + char_width > width && !current.is_empty() {
            parts.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(character);
        current_width += char_width;
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn section_label(label: &str, width: usize) -> String {
    let content = format!("{} ", label);
    let remaining = width.saturating_sub(visible_width(&content));
    format!("{}{}", content, "─".repeat(remaining))
}

fn preview_section_label(provider: Option<&ProviderState>) -> &'static str {
    match provider.map(|provider| &provider.load_state) {
        Some(ProviderLoadState::Error(_)) => "Error",
        _ => "Preview",
    }
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

fn maybe_style_entry(provider: Option<&ProviderState>, text: &str, selected: bool) -> String {
    let styled = match syntax_flavor(provider) {
        SyntaxFlavor::Python => highlight_python(text),
        SyntaxFlavor::Shell => highlight_shell(text),
        SyntaxFlavor::Plain => text.to_owned(),
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

enum SyntaxFlavor {
    Plain,
    Python,
    Shell,
}

fn syntax_flavor(provider: Option<&ProviderState>) -> SyntaxFlavor {
    match provider.map(|provider| &provider.config.kind) {
        Some(ProviderKind::IPython(_)) => SyntaxFlavor::Python,
        Some(ProviderKind::FileLines(_)) => SyntaxFlavor::Shell,
        Some(ProviderKind::Command(config))
            if matches!(config.output_mode, CommandOutputMode::Lines) =>
        {
            SyntaxFlavor::Shell
        }
        _ => SyntaxFlavor::Plain,
    }
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

fn highlight_shell(text: &str) -> String {
    let mut rendered = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let mut expect_command = true;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            rendered.push(c);
            i += 1;
            continue;
        }

        if c == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
            let rest: String = chars[i..].iter().collect();
            rendered.push_str(ANSI_BRIGHT_BLACK);
            rendered.push_str(ANSI_ITALIC);
            rendered.push_str(&rest);
            rendered.push_str(ANSI_RESET);
            break;
        }

        if c == '\'' || c == '"' {
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
            expect_command = false;
            continue;
        }

        if c == '$' {
            let start = i;
            i += 1;
            if i < chars.len() && chars[i] == '{' {
                i += 1;
                while i < chars.len() && chars[i] != '}' {
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }
            } else {
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
            }
            let token: String = chars[start..i].iter().collect();
            rendered.push_str(ANSI_CYAN);
            rendered.push_str(&token);
            rendered.push_str(ANSI_RESET);
            expect_command = false;
            continue;
        }

        if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            rendered.push_str(ANSI_CYAN);
            rendered.push_str(&token);
            rendered.push_str(ANSI_RESET);
            expect_command = false;
            continue;
        }

        if matches!(c, '|' | '&' | ';' | '<' | '>') {
            let start = i;
            i += 1;
            while i < chars.len() && matches!(chars[i], '|' | '&' | '<' | '>') {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            rendered.push_str(ANSI_YELLOW);
            rendered.push_str(&token);
            rendered.push_str(ANSI_RESET);
            expect_command = true;
            continue;
        }

        if is_shell_word_start(c) {
            let start = i;
            i += 1;
            while i < chars.len() && is_shell_word_char(chars[i]) {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();

            if i < chars.len()
                && chars[i] == '='
                && is_env_assignment_name(&token)
                && expect_command
            {
                rendered.push_str(ANSI_CYAN);
                rendered.push_str(&token);
                rendered.push_str(ANSI_RESET);
                rendered.push('=');
                i += 1;
                continue;
            }

            if expect_command && !token.starts_with('-') {
                rendered.push_str(ANSI_BLUE);
                rendered.push_str(ANSI_BOLD);
                rendered.push_str(&token);
                rendered.push_str(ANSI_RESET);
            } else {
                rendered.push_str(&token);
            }
            expect_command = false;
            continue;
        }

        rendered.push(c);
        i += 1;
    }

    rendered
}

fn is_shell_word_start(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '/' | '~' | '-')
}

fn is_shell_word_char(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '_' | '.' | '/' | ':' | '~' | '-' | '+' | '%')
}

fn is_env_assignment_name(token: &str) -> bool {
    !token.is_empty()
        && token.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}
