//! Snippet extraction and copy menu for the last agent response.

use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use ratatui::style::Stylize;

use super::ChatWidget;
use crate::history_cell;

impl ChatWidget {
    /// Opens a copy menu for inline-code snippets in the last agent response.
    pub(crate) fn open_snippet_picker(&mut self) {
        self.open_snippet_menu_from_response(/*response_offset*/ 0);
    }

    /// Opens a copy menu for the next recent agent response that has snippets.
    pub(crate) fn open_snippet_menu_from_response(&mut self, response_offset: usize) {
        let markdowns = self.snippet_response_markdowns();
        if markdowns.is_empty() {
            self.add_to_history(history_cell::new_error_event(
                "No agent response to inspect for snippets".into(),
            ));
            self.request_redraw();
            return;
        }

        let Some((response_offset, snippets)) = (0..markdowns.len()).find_map(|offset| {
            let response_offset = (response_offset + offset) % markdowns.len();
            let snippets = snippets_from_markdown(markdowns[response_offset]);
            (!snippets.is_empty()).then_some((response_offset, snippets))
        }) else {
            self.add_to_history(history_cell::new_info_event(
                "No snippets in recent responses".into(),
                /*hint*/ None,
            ));
            self.request_redraw();
            return;
        };

        self.bottom_pane
            .show_snippet_menu(snippets, response_offset);
        self.request_redraw();
    }

    fn snippet_response_markdowns(&self) -> Vec<&str> {
        let retained = self
            .transcript
            .agent_turn_markdowns
            .iter()
            .rev()
            .map(|entry| entry.markdown.as_str())
            .filter(|markdown| !markdown.is_empty())
            .collect::<Vec<_>>();
        if !retained.is_empty() {
            return retained;
        }
        self.transcript
            .last_agent_markdown
            .as_deref()
            .into_iter()
            .collect()
    }

    /// Copies one snippet selected from the picker.
    pub(crate) fn copy_snippet_to_clipboard(&mut self, snippet: String) {
        self.copy_snippet_to_clipboard_with(snippet, crate::clipboard_copy::copy_to_clipboard);
    }

    pub(super) fn copy_snippet_to_clipboard_with(
        &mut self,
        snippet: String,
        copy_fn: impl FnOnce(&str) -> Result<Option<crate::clipboard_copy::ClipboardLease>, String>,
    ) {
        if snippet.is_empty() {
            self.add_to_history(history_cell::new_error_event(
                "Selected snippet is empty".into(),
            ));
            self.request_redraw();
            return;
        }

        match copy_fn(&snippet) {
            Ok(lease) => {
                self.clipboard_lease = lease;
                self.add_to_history(history_cell::PlainHistoryCell::new(vec![
                    vec![
                        "• ".dim(),
                        "Copied snippet to clipboard: ".into(),
                        snippet.cyan(),
                    ]
                    .into(),
                ]));
            }
            Err(error) => self.add_to_history(history_cell::new_error_event(format!(
                "Copy failed: {error}"
            ))),
        }
        self.request_redraw();
    }
}

fn snippets_from_markdown(markdown: &str) -> Vec<String> {
    let mut snippets = Parser::new(markdown)
        .into_offset_iter()
        .filter_map(|(event, _range)| match event {
            Event::Code(code) if !code.trim().is_empty() => Some(code.into_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    snippets.sort();
    snippets.dedup();
    snippets
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn snippets_from_markdown_extracts_sorted_unique_inline_code() {
        let snippets = snippets_from_markdown(
            "See `tui/src/keymap.rs`.\n\nRun `cargo test -p codex-tui`, then run `cargo test -p codex-tui` again.",
        );

        assert_eq!(
            snippets,
            vec![
                "cargo test -p codex-tui".to_string(),
                "tui/src/keymap.rs".to_string()
            ]
        );
    }

    #[test]
    fn snippets_from_markdown_skips_fenced_code_blocks() {
        let snippets = snippets_from_markdown(
            "`inline`\n\n```sh\ncargo test -p codex-tui\n```\n\n`` still inline ``",
        );

        assert_eq!(
            snippets,
            vec!["inline".to_string(), "still inline".to_string()]
        );
    }
}
