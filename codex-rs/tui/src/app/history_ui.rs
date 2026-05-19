//! Terminal history and clear-screen UI helpers for the TUI app.
//!
//! This module owns rendering the fresh session header, clearing inline or alternate-screen UI
//! state, and resetting transcript-related app state after `/clear` or Ctrl-L.

use super::*;

impl App {
    pub(super) fn open_url_in_browser(&mut self, url: String) {
        if let Err(err) = webbrowser::open(&url) {
            self.chat_widget
                .add_error_message(format!("Failed to open browser for {url}: {err}"));
            return;
        }

        self.chat_widget
            .add_info_message(format!("Opened {url} in your browser."), /*hint*/ None);
    }

    pub(super) fn clear_ui_header_lines_with_version(
        &self,
        width: u16,
        version: &'static str,
    ) -> Vec<Line<'static>> {
        let _ = (width, version);
        vec![Line::from("")]
    }

    pub(super) fn clear_ui_header_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.clear_ui_header_lines_with_version(width, CODEX_CLI_VERSION)
    }

    pub(super) fn queue_clear_ui_header(&mut self, tui: &mut tui::Tui) {
        let width = tui.terminal.last_known_screen_size.width;
        let header_lines = self.clear_ui_header_lines(width);
        if !header_lines.is_empty() {
            tui.insert_history_lines(header_lines);
            self.has_emitted_history_lines = true;
        }
    }

    pub(super) fn clear_terminal_ui(
        &mut self,
        tui: &mut tui::Tui,
        redraw_header: bool,
    ) -> Result<()> {
        let is_alt_screen_active = tui.is_alt_screen_active();

        // Drop queued history insertions so stale transcript lines cannot be flushed after /clear.
        tui.clear_pending_history_lines();

        if is_alt_screen_active {
            tui.terminal.clear_visible_screen()?;
        } else {
            // Some terminals (Terminal.app, Warp) do not reliably drop scrollback when purge and
            // clear are emitted as separate backend commands. Prefer a single ANSI sequence.
            tui.terminal.clear_scrollback_and_visible_screen_ansi()?;
        }

        let mut area = tui.terminal.viewport_area;
        if area.y > 0 {
            // After a full clear, anchor the inline viewport at the top.
            area.y = 0;
            tui.terminal.set_viewport_area(area);
        }
        self.has_emitted_history_lines = false;

        if redraw_header {
            self.queue_clear_ui_header(tui);
        }
        Ok(())
    }

    pub(super) fn reset_app_ui_state_after_clear(&mut self) {
        self.reset_transcript_state_after_clear();
    }

    pub(super) fn reset_transcript_state_after_clear(&mut self) {
        self.overlay = None;
        self.transcript_cells.clear();
        self.deferred_history_lines.clear();
        self.has_emitted_history_lines = false;
        self.transcript_reflow.clear();
        self.initial_history_replay_buffer = None;
        self.backtrack = BacktrackState::default();
        self.chat_widget.clear_edit_last_message_hint();
        self.backtrack_render_pending = false;
    }

    /// Toggle native main-view scrollback between full and message-only transcript projections.
    ///
    /// The canonical `transcript_cells` collection remains untouched. We clear the terminal's
    /// rendered scrollback, flip the projection bit, and replay the current thread into ordinary
    /// terminal history so native scrolling resumes immediately afterward.
    pub(super) fn toggle_condensed_transcript_view(&mut self, tui: &mut tui::Tui) -> Result<()> {
        self.condensed_transcript_view = !self.condensed_transcript_view;
        self.chat_widget
            .set_condensed_transcript_view(self.condensed_transcript_view);
        self.deferred_history_lines.clear();
        self.initial_history_replay_buffer = None;
        // The toggle performs its own authoritative clear + replay. Drop any older resize-reflow
        // repaint request so it cannot immediately replace the freshly restored projection with a
        // stale/capped replay scheduled before the mode switch.
        self.transcript_reflow.clear();
        self.clear_terminal_ui(tui, /*redraw_header*/ false)?;
        self.render_transcript_once(tui);
        Ok(())
    }
}
