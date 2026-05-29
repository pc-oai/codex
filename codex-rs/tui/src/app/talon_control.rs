//! Talon state snapshots and per-session command handling.

use super::*;

impl App {
    const TALON_RECENT_MESSAGE_LIMIT: usize = 8;

    fn select_model_from_talon_command(
        &mut self,
        model: String,
        requested_effort: Option<ReasoningEffortConfig>,
    ) -> bool {
        let models = self.model_catalog.try_list_models().unwrap_or_default();
        let Some(preset) = models
            .into_iter()
            .find(|preset| preset.show_in_picker && preset.model == model)
        else {
            return false;
        };

        let selected_effort = requested_effort
            .filter(|effort| {
                preset
                    .supported_reasoning_efforts
                    .iter()
                    .any(|option| option.effort == *effort)
            })
            .or(Some(preset.default_reasoning_effort));

        let previous_model = self.chat_widget.current_model().to_string();
        self.chat_widget.set_model(&model);
        self.on_update_reasoning_effort(selected_effort);
        self.app_event_tx.send(AppEvent::PersistModelSelection {
            previous_model,
            model,
            effort: selected_effort,
        });
        true
    }

    pub(super) fn talon_ambient_state(&self) -> crate::talon::TalonAmbientState {
        let models = self
            .model_catalog
            .try_list_models()
            .unwrap_or_default()
            .into_iter()
            .map(|preset| crate::talon::TalonModelState {
                model: preset.model,
                display_name: preset.display_name,
                default_reasoning_effort: preset.default_reasoning_effort,
                supported_reasoning_efforts: preset
                    .supported_reasoning_efforts
                    .into_iter()
                    .map(|option| option.effort)
                    .collect(),
                show_in_picker: preset.show_in_picker,
            })
            .collect();

        crate::talon::TalonAmbientState {
            version: 1,
            session_id: self.chat_widget.thread_id().map(|id| id.to_string()),
            local_build_number: crate::version::local_build_number(),
            is_task_running: self.chat_widget.is_task_running(),
            last_user_request: self.latest_user_request_text(),
            recent_user_requests: self.recent_user_request_texts(Self::TALON_RECENT_MESSAGE_LIMIT),
            recent_agent_responses: self
                .chat_widget
                .recent_agent_markdowns(Self::TALON_RECENT_MESSAGE_LIMIT),
            current_model: self.chat_widget.current_model().to_string(),
            current_reasoning_effort: self.chat_widget.current_reasoning_effort(),
            models,
            timestamp_ms: crate::talon::now_timestamp_ms(),
        }
    }

    pub(super) fn write_talon_ambient_state(&self, paths: &crate::talon::TalonPaths) {
        let _ = crate::talon::write_state(paths, &self.talon_ambient_state());
    }

    pub(super) async fn handle_talon_request(
        &mut self,
        tui: &mut tui::Tui,
        req: crate::talon::TalonRequest,
    ) -> crate::talon::TalonResponse {
        let mut applied = Vec::new();

        for cmd in req.commands {
            use crate::talon::TalonCommand::*;
            match cmd {
                SetBuffer { text, cursor } => {
                    self.chat_widget
                        .set_composer_text(text, Vec::new(), Vec::new());
                    if let Some(pos) = cursor {
                        self.chat_widget.set_composer_cursor(pos);
                    }
                    applied.push("set_buffer".to_string());
                }
                SetCursor { cursor } => {
                    self.chat_widget.set_composer_cursor(cursor);
                    applied.push("set_cursor".to_string());
                }
                GetState => {
                    applied.push("get_state".to_string());
                }
                Notify { message } => {
                    let _ = tui.notify(message);
                    applied.push("notify".to_string());
                }
                EditPreviousMessage { steps_back } => {
                    if self.chat_widget.history_edit_previous(steps_back) {
                        applied.push("edit_previous_message".to_string());
                    }
                }
                EditLastMessage => {
                    let edited = self.edit_last_message_from_command();
                    tui.frame_requester().schedule_frame();
                    if edited {
                        applied.push("edit_last_message".to_string());
                    }
                }
                CopyLastRequest => {
                    let request = self.latest_user_request_text();
                    self.chat_widget.copy_last_user_request_text(request);
                    applied.push("copy_last_request".to_string());
                }
                CopyLastResponse => {
                    self.chat_widget.copy_last_agent_markdown();
                    applied.push("copy_last_response".to_string());
                }
                RetitleCurrentSession => {
                    self.chat_widget.request_retitle_suggestion();
                    applied.push("retitle_current_session".to_string());
                }
                RenameCurrentSession { name } => {
                    if self.chat_widget.rename_thread_from_text(&name) {
                        applied.push("rename_current_session".to_string());
                    }
                }
                EmojiCurrentSession => {
                    self.chat_widget.request_emoji_suggestion();
                    applied.push("emoji_current_session".to_string());
                }
                ParkCurrentSession => {
                    self.app_event_tx
                        .set_thread_user_state(codex_app_server_protocol::ThreadUserState::Parked);
                    applied.push("park_current_session".to_string());
                }
                DoneCurrentSession => {
                    self.app_event_tx
                        .set_thread_user_state(codex_app_server_protocol::ThreadUserState::Done);
                    applied.push("done_current_session".to_string());
                }
                ActivateCurrentSession => {
                    self.app_event_tx
                        .set_thread_user_state(codex_app_server_protocol::ThreadUserState::Active);
                    applied.push("activate_current_session".to_string());
                }
                InterruptCurrentTurn => {
                    if self.chat_widget.is_task_running() {
                        self.app_event_tx
                            .send(AppEvent::CodexOp(AppCommand::Interrupt));
                        applied.push("interrupt_current_turn".to_string());
                    } else {
                        applied.push("interrupt_current_turn_skipped_idle".to_string());
                    }
                }
                ExitCurrentSession => {
                    self.app_event_tx
                        .send(AppEvent::Exit(ExitMode::ShutdownFirst));
                    applied.push("exit_current_session".to_string());
                }
                SetModel { model, effort } => {
                    if self.select_model_from_talon_command(model, effort) {
                        applied.push("set_model".to_string());
                    }
                }
                ReloadCurrentSessionIfIdle => {
                    if self.chat_widget.is_task_running() {
                        applied.push("reload_current_session_if_idle_skipped_busy".to_string());
                    } else if self.agent_tree_is_busy().await {
                        applied
                            .push("reload_current_session_if_idle_skipped_tree_busy".to_string());
                    } else {
                        self.app_event_tx.send(AppEvent::ReloadCurrentSession);
                        applied.push("reload_current_session_if_idle".to_string());
                    }
                }
                ReloadCurrentSession => {
                    self.app_event_tx.send(AppEvent::ReloadCurrentSession);
                    applied.push("reload_current_session".to_string());
                }
                HistoryPrevious => {
                    if self.chat_widget.history_previous() {
                        applied.push("history_previous".to_string());
                    }
                }
                HistoryNext => {
                    if self.chat_widget.history_next() {
                        applied.push("history_next".to_string());
                    }
                }
            }
        }

        let state = crate::talon::TalonEditorState {
            buffer: self.chat_widget.composer_text(),
            cursor: self.chat_widget.composer_cursor(),
            is_task_running: self.chat_widget.is_task_running(),
            task_summary: crate::talon::status_summary(),
            session_id: self.chat_widget.thread_id().map(|id| id.to_string()),
            cwd: Some(self.config.cwd.display().to_string()),
            last_user_request: self.latest_user_request_text(),
            recent_user_requests: self.recent_user_request_texts(Self::TALON_RECENT_MESSAGE_LIMIT),
            recent_agent_responses: self
                .chat_widget
                .recent_agent_markdowns(Self::TALON_RECENT_MESSAGE_LIMIT),
            model: self.chat_widget.current_model().to_string(),
            reasoning_effort: self.chat_widget.current_reasoning_effort(),
            local_build_number: crate::version::local_build_number(),
        };

        crate::talon::TalonResponse {
            version: 1,
            status: crate::talon::TalonResponseStatus::Ok,
            state,
            applied,
            error: None,
            timestamp_ms: crate::talon::now_timestamp_ms(),
        }
    }
}
