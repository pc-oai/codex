//! Helpers for generated thread-title metadata.
//!
//! `/retitle` and `/emoji` ask the model for thread metadata, but that answer should not become
//! part of the user's real conversation. This module keeps the title-composition rules shared
//! between the slash-command surface and the app-level out-of-band worker flow.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThreadNameSuggestionKind {
    Retitle,
    Emoji,
    EmojiWithTitle,
}

pub(crate) fn compose_thread_name(
    kind: ThreadNameSuggestionKind,
    current_name: Option<&str>,
    suggestion: &str,
) -> Option<String> {
    let suggestion = first_suggestion_line(suggestion)?;
    let next_name = match kind {
        ThreadNameSuggestionKind::Retitle => {
            let emoji = current_name
                .and_then(leading_emoji_prefix)
                .unwrap_or_default();
            format!("{emoji}{suggestion}")
        }
        ThreadNameSuggestionKind::Emoji => {
            let title = current_name
                .and_then(strip_leading_emoji_prefix)
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| "Untitled session".to_string());
            format!("{suggestion} {title}")
        }
        ThreadNameSuggestionKind::EmojiWithTitle => suggestion,
    };
    crate::legacy_core::util::normalize_thread_name(&next_name)
}

pub(crate) fn thread_name_is_meaningful(thread_name: Option<&str>) -> bool {
    thread_name
        .and_then(strip_leading_emoji_prefix)
        .is_some_and(|title| !title.is_empty() && title != "Untitled session")
}

fn first_suggestion_line(message: &str) -> Option<String> {
    message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_matches(['"', '\'', '`']).trim().to_string())
        .filter(|line| !line.is_empty())
}

fn leading_emoji_prefix(title: &str) -> Option<String> {
    let trimmed = title.trim();
    let prefix_len = leading_emoji_prefix_len(trimmed)?;
    Some(format!("{} ", trimmed[..prefix_len].trim_end()))
}

fn strip_leading_emoji_prefix(title: &str) -> Option<String> {
    let trimmed = title.trim();
    if let Some(prefix_len) = leading_emoji_prefix_len(trimmed) {
        Some(trimmed[prefix_len..].trim_start().to_string())
    } else {
        Some(trimmed.to_string())
    }
}

fn leading_emoji_prefix_len(title: &str) -> Option<usize> {
    let mut seen = 0usize;
    let mut end = 0usize;
    for (idx, token) in title.split_whitespace().enumerate() {
        if idx >= 3 || !is_emoji_token(token) {
            break;
        }
        let token_start = title[end..].find(token).map(|offset| end + offset)?;
        end = token_start + token.len();
        seen += 1;
    }
    (seen > 0).then_some(end)
}

fn is_emoji_token(token: &str) -> bool {
    !token.is_ascii() && !token.chars().any(char::is_alphanumeric)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn retitle_preserves_existing_emoji_prefix() {
        assert_eq!(
            compose_thread_name(
                ThreadNameSuggestionKind::Retitle,
                Some("🧭 🗂️ Old title"),
                "Better title",
            ),
            Some("🧭 🗂️ Better title".to_string())
        );
    }

    #[test]
    fn emoji_replaces_existing_prefix_without_rewriting_title() {
        assert_eq!(
            compose_thread_name(
                ThreadNameSuggestionKind::Emoji,
                Some("🧭 🗂️ Existing title"),
                "🧪 🧭 🔎",
            ),
            Some("🧪 🧭 🔎 Existing title".to_string())
        );
    }

    #[test]
    fn emoji_with_title_uses_full_suggestion() {
        assert_eq!(
            compose_thread_name(
                ThreadNameSuggestionKind::EmojiWithTitle,
                None,
                "🔄🩺 Full reconcile monitoring",
            ),
            Some("🔄🩺 Full reconcile monitoring".to_string())
        );
    }
}
