use crate::model::{HistoryEntry, MatchResult};

pub fn filter_entries(
    entries: &[HistoryEntry],
    query: &str,
    case_sensitive: bool,
    max_results: usize,
) -> Vec<MatchResult> {
    if entries.is_empty() || max_results == 0 {
        return Vec::new();
    }

    let query = query.trim();
    if query.is_empty() {
        return entries
            .iter()
            .enumerate()
            .take(max_results)
            .map(|(entry_index, entry)| MatchResult {
                entry_index,
                score: entry.score_hint.unwrap_or(0) + (entries.len().saturating_sub(entry_index) as i64),
            })
            .collect();
    }

    let query_tokens = query
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| normalize(token, case_sensitive))
        .collect::<Vec<_>>();

    let mut matches = Vec::new();
    for (entry_index, entry) in entries.iter().enumerate() {
        if let Some(score) = score_entry(entry, &query_tokens, case_sensitive) {
            matches.push(MatchResult { entry_index, score });
        }
    }

    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.entry_index.cmp(&right.entry_index))
    });
    matches.truncate(max_results);
    matches
}

fn score_entry(entry: &HistoryEntry, query_tokens: &[String], case_sensitive: bool) -> Option<i64> {
    let haystack = normalize(&entry.text, case_sensitive);
    let mut total = entry.score_hint.unwrap_or(0);

    for token in query_tokens {
        let token_score = score_token(&haystack, token)?;
        total += token_score;
    }

    if haystack.starts_with(&query_tokens.join(" ")) {
        total += 200;
    }

    Some(total)
}

fn score_token(haystack: &str, token: &str) -> Option<i64> {
    if token.is_empty() {
        return Some(0);
    }
    if let Some(position) = haystack.find(token) {
        return Some(1_000 - position as i64);
    }

    let mut total = 0i64;
    let mut last_match = None;
    let mut search_from = 0usize;
    for needle_char in token.chars() {
        let found = haystack[search_from..]
            .char_indices()
            .find_map(|(offset, candidate)| (candidate == needle_char).then_some(search_from + offset));
        let position = found?;
        total += 25;
        if let Some(last_match) = last_match {
            if position == last_match + 1 {
                total += 15;
            }
        } else if position == 0 {
            total += 80;
        }
        if position > 0 && haystack.as_bytes()[position - 1].is_ascii_whitespace() {
            total += 40;
        }
        total -= position as i64;
        last_match = Some(position);
        search_from = position + needle_char.len_utf8();
    }

    Some(total)
}

fn normalize(value: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        value.to_owned()
    } else {
        value.to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::filter_entries;
    use crate::model::HistoryEntry;
    use std::collections::BTreeMap;

    fn entry(text: &str) -> HistoryEntry {
        HistoryEntry {
            id: text.to_owned(),
            provider_name: "test".to_owned(),
            text: text.to_owned(),
            preview: None,
            timestamp: None,
            score_hint: None,
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn prefers_prefix_matches() {
        let entries = vec![entry("git status"), entry("status git")];
        let matches = filter_entries(&entries, "git st", false, 10);
        assert_eq!(matches[0].entry_index, 0);
    }
}
