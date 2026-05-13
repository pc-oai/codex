//! Keyboard input, external editor, and status-line dispatch for the TUI app.
//!
//! This module owns global key bindings that sit above ChatWidget, including transcript overlay
//! entry, Ctrl-L clear, external editor launch, and agent navigation shortcuts.

use super::*;

impl App {
    pub(super) async fn launch_external_editor(&mut self, tui: &mut tui::Tui) {
        let editor_cmd = match external_editor::resolve_editor_command() {
            Ok(cmd) => cmd,
            Err(external_editor::EditorError::MissingEditor) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(
                    "Cannot open external editor: set $VISUAL or $EDITOR before starting Codex."
                        .to_string(),
                ));
                self.reset_external_editor_state(tui);
                return;
            }
            Err(err) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(format!(
                        "Failed to open editor: {err}",
                    )));
                self.reset_external_editor_state(tui);
                return;
            }
        };

        let seed = self.chat_widget.composer_text_with_pending();
        let editor_result = tui
            .with_restored(tui::RestoreMode::KeepRaw, || async {
                external_editor::run_editor(&seed, &editor_cmd).await
            })
            .await;
        self.reset_external_editor_state(tui);

        match editor_result {
            Ok(new_text) => {
                // Trim trailing whitespace
                let cleaned = new_text.trim_end().to_string();
                self.chat_widget.apply_external_edit(cleaned);
            }
            Err(err) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(format!(
                        "Failed to open editor: {err}",
                    )));
            }
        }
        tui.frame_requester().schedule_frame();
    }

    pub(super) fn request_external_editor_launch(&mut self, tui: &mut tui::Tui) {
        self.chat_widget
            .set_external_editor_state(ExternalEditorState::Requested);
        self.chat_widget.set_footer_hint_override(Some(vec![(
            EXTERNAL_EDITOR_HINT.to_string(),
            String::new(),
        )]));
        tui.frame_requester().schedule_frame();
    }

    pub(super) fn reset_external_editor_state(&mut self, tui: &mut tui::Tui) {
        self.chat_widget
            .set_external_editor_state(ExternalEditorState::Closed);
        self.chat_widget.set_footer_hint_override(/*items*/ None);
        tui.frame_requester().schedule_frame();
    }

    pub(super) async fn handle_key_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        key_event: KeyEvent,
    ) {
        // Some terminals, especially on macOS, encode Option+Left/Right as Option+b/f unless
        // enhanced keyboard reporting is available. We only treat those word-motion fallbacks as
        // agent-switch shortcuts when the composer is empty so we never steal the expected
        // editing behavior for moving across words inside a draft.
        let allow_agent_word_motion_fallback = !self.enhanced_keys_supported
            && self.chat_widget.composer_text_with_pending().is_empty();
        if self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            // Alt+Left/Right are also natural word-motion keys in the composer. Keep agent
            // fast-switch available only once the draft is empty so editing behavior wins whenever
            // there is text on screen.
            && self.chat_widget.composer_text_with_pending().is_empty()
            && previous_agent_shortcut_matches(key_event, allow_agent_word_motion_fallback)
        {
            if let Some(thread_id) = self
                .adjacent_thread_id_with_backfill(app_server, AgentNavigationDirection::Previous)
                .await
            {
                let _ = self
                    .select_agent_thread_and_discard_side(tui, app_server, thread_id)
                    .await;
            }
            return;
        }
        if self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            // Mirror the previous-agent rule above: empty drafts may use these keys for thread
            // switching, but non-empty drafts keep them for expected word-wise cursor motion.
            && self.chat_widget.composer_text_with_pending().is_empty()
            && next_agent_shortcut_matches(key_event, allow_agent_word_motion_fallback)
        {
            if let Some(thread_id) = self
                .adjacent_thread_id_with_backfill(app_server, AgentNavigationDirection::Next)
                .await
            {
                let _ = self
                    .select_agent_thread_and_discard_side(tui, app_server, thread_id)
                    .await;
            }
            return;
        }
        if side_return_shortcut_matches(key_event)
            && self.maybe_return_from_side(tui, app_server).await
        {
            return;
        }

        let app_keymap_shortcuts_available = self.app_keymap_shortcuts_available();

        if app_keymap_shortcuts_available && self.keymap.app.toggle_vim_mode.is_pressed(key_event) {
            self.chat_widget.toggle_vim_mode_and_notify();
            return;
        }

        if app_keymap_shortcuts_available
            && self.keymap.app.toggle_fast_mode.is_pressed(key_event)
            && self.chat_widget.can_toggle_fast_mode_from_keybinding()
        {
            self.chat_widget.toggle_fast_mode_from_ui();
            return;
        }

        if self.should_step_edit_last_message_preview_older(key_event) {
            self.step_backtrack_edit_preview_older();
            tui.frame_requester().schedule_frame();
            return;
        }

        if self.should_interrupt_turn_for_edit_last_message_shortcut(key_event) {
            self.interrupt_turn_then_edit_last_message();
            tui.frame_requester().schedule_frame();
            return;
        }

        if self.should_handle_edit_last_message_shortcut(key_event) {
            self.edit_last_message_from_command();
            tui.frame_requester().schedule_frame();
            return;
        }

        if self.should_cancel_edit_last_message_preview(key_event) {
            self.cancel_backtrack_edit_preview();
            tui.frame_requester().schedule_frame();
            return;
        }

        if self.should_commit_edit_last_message_preview(key_event) {
            self.commit_backtrack_edit_preview();
            tui.frame_requester().schedule_frame();
            return;
        }

        if app_keymap_shortcuts_available && self.keymap.app.open_transcript.is_pressed(key_event) {
            // Enter alternate screen and set viewport to full size.
            let _ = tui.enter_alt_screen();
            self.overlay = Some(Overlay::new_transcript(
                self.transcript_cells.clone(),
                self.keymap.pager.clone(),
            ));
            tui.frame_requester().schedule_frame();
            return;
        }

        if app_keymap_shortcuts_available
            && self.keymap.app.open_external_editor.is_pressed(key_event)
        {
            // Only launch the external editor if there is no overlay and the bottom pane is not in use.
            // Note that it can be launched while a task is running to enable editing while the previous turn is ongoing.
            if self.overlay.is_none()
                && self.chat_widget.can_launch_external_editor()
                && self.chat_widget.external_editor_state() == ExternalEditorState::Closed
            {
                self.request_external_editor_launch(tui);
            }
            return;
        }

        if matches!(key_event.code, KeyCode::Esc)
            && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            // Esc primes/advances backtracking only in normal (not working) mode
            // with the composer focused and empty. In any other state, forward
            // Esc so the active UI (e.g. status indicator, modals, popups)
            // handles it.
            if self.should_handle_backtrack_esc(key_event) {
                self.handle_backtrack_esc_key(tui);
            } else {
                self.chat_widget.handle_key_event(key_event);
            }
            return;
        }

        match key_event {
            _ if app_keymap_shortcuts_available
                && self.keymap.app.clear_terminal.is_pressed(key_event) =>
            {
                if !self.chat_widget.can_run_ctrl_l_clear_now() {
                    return;
                }
                if let Err(err) = self.clear_terminal_ui(tui, /*redraw_header*/ false) {
                    tracing::warn!(error = %err, "failed to clear terminal UI");
                    self.chat_widget
                        .add_error_message(format!("Failed to clear terminal UI: {err}"));
                } else {
                    self.reset_app_ui_state_after_clear();
                    self.queue_clear_ui_header(tui);
                    tui.frame_requester().schedule_frame();
                }
            }
            // Enter confirms backtrack when primed + count > 0. Otherwise pass to widget.
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } if self.backtrack.primed
                && self.backtrack.nth_user_message != usize::MAX
                && self.chat_widget.composer_is_empty() =>
            {
                if let Some(selection) = self.confirm_backtrack_from_main() {
                    self.apply_backtrack_selection(tui, selection);
                }
            }
            KeyEvent {
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                // Any non-Esc key press should cancel a primed backtrack.
                // This avoids stale "Esc-primed" state after the user starts typing
                // (even if they later backspace to empty).
                if key_event.code != KeyCode::Esc && self.backtrack.primed {
                    self.reset_backtrack_state();
                }
                self.chat_widget.handle_key_event(key_event);
            }
            _ => {
                self.chat_widget.handle_key_event(key_event);
            }
        };
    }

    pub(super) fn should_handle_backtrack_esc(&self, key_event: KeyEvent) -> bool {
        self.chat_widget.is_normal_backtrack_mode()
            && self.chat_widget.composer_is_empty()
            && !self.chat_widget.should_handle_vim_insert_escape(key_event)
    }

    /// Reuse the queued-message edit shortcut for the idle main view.
    ///
    /// When a task is running, ChatWidget still owns this binding so it can pull a queued follow-up
    /// back into the composer. Once idle, the same gesture jumps straight to editing the last sent
    /// message instead of forcing the older Esc-Esc-Enter sequence. That edit intentionally
    /// replaces any visible draft already in the composer.
    pub(super) fn should_handle_edit_last_message_shortcut(&self, key_event: KeyEvent) -> bool {
        key_event.kind == KeyEventKind::Press
            && self.app_keymap_shortcuts_available()
            && self.keymap.chat.edit_queued_message.is_pressed(key_event)
            && self.chat_widget.is_normal_backtrack_mode()
    }

    /// During a live turn, Ctrl-E should stop the turn so the sent request can be edited next.
    ///
    /// Queued follow-ups keep their existing priority: when one exists, ChatWidget still owns
    /// Ctrl-E so it can pull that queued draft back into the composer instead. Otherwise, the
    /// eventual edit preview replaces any visible composer draft.
    pub(super) fn should_interrupt_turn_for_edit_last_message_shortcut(
        &self,
        key_event: KeyEvent,
    ) -> bool {
        key_event.kind == KeyEventKind::Press
            && self.app_keymap_shortcuts_available()
            && self.keymap.chat.edit_queued_message.is_pressed(key_event)
            && self.chat_widget.is_task_running()
            && !self.chat_widget.has_queued_follow_up_messages()
    }

    fn should_step_edit_last_message_preview_older(&self, key_event: KeyEvent) -> bool {
        key_event.kind == KeyEventKind::Press
            && self.app_keymap_shortcuts_available()
            && self.keymap.chat.edit_queued_message.is_pressed(key_event)
            && self.backtrack_edit_preview_active()
            && self.chat_widget.is_normal_backtrack_mode()
    }

    fn should_cancel_edit_last_message_preview(&self, key_event: KeyEvent) -> bool {
        matches!(key_event.code, KeyCode::Esc)
            && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && self.backtrack_edit_preview_active()
            && self.chat_widget.no_modal_or_popup_active()
            && !self.chat_widget.should_handle_vim_insert_escape(key_event)
    }

    fn should_commit_edit_last_message_preview(&self, key_event: KeyEvent) -> bool {
        matches!(key_event.code, KeyCode::Enter)
            && key_event.kind == KeyEventKind::Press
            && self.backtrack_edit_preview_active()
            && self.chat_widget.no_modal_or_popup_active()
    }

    fn app_keymap_shortcuts_available(&self) -> bool {
        self.overlay.is_none() && self.chat_widget.no_modal_or_popup_active()
    }

    pub(super) fn refresh_status_line(&mut self) {
        self.chat_widget.refresh_status_line();
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::make_test_app;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    #[tokio::test]
    async fn app_keymap_shortcuts_are_disabled_while_keymap_view_is_active() {
        let mut app = make_test_app().await;
        assert!(app.app_keymap_shortcuts_available());

        let keymap = app.keymap.clone();
        app.chat_widget.open_keymap_debug(&keymap);

        assert!(!app.app_keymap_shortcuts_available());
    }

    #[tokio::test]
    async fn edit_queued_shortcut_becomes_edit_last_message_when_idle() {
        let app = make_test_app().await;
        let alt_up = KeyEvent::new(KeyCode::Up, KeyModifiers::ALT);
        let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);

        assert!(app.should_handle_edit_last_message_shortcut(alt_up));
        assert!(app.should_handle_edit_last_message_shortcut(ctrl_e));
    }

    #[tokio::test]
    async fn edit_last_message_shortcut_replaces_nonempty_idle_composer() {
        let mut app = make_test_app().await;
        let alt_up = KeyEvent::new(KeyCode::Up, KeyModifiers::ALT);

        app.chat_widget
            .set_composer_text("draft".to_string(), Vec::new(), Vec::new());
        assert!(app.should_handle_edit_last_message_shortcut(alt_up));
    }
}
