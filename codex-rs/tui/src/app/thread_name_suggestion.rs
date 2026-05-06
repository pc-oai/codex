//! Out-of-band generated thread-title metadata.
//!
//! `/retitle` and `/emoji` should read the whole conversation without adding their own prompts or
//! answers to the visible thread. The app owns that lifecycle by forking an ephemeral hidden child,
//! running one constrained turn there, then applying only the resulting title to the parent.

use super::*;
use crate::thread_name_suggestion::ThreadNameSuggestionKind;
use crate::thread_name_suggestion::compose_thread_name;
use codex_protocol::models::MessagePhase;

const THREAD_NAME_SUGGESTION_DEVELOPER_INSTRUCTIONS: &str = r#"You are generating metadata for an existing thread, not continuing the user's conversation.

The inherited conversation history is reference material only. Do not continue tasks, execute plans, call tools, modify files, or ask follow-up questions from that history.

Only answer the latest user prompt. Follow its output format exactly."#;

#[derive(Clone, Debug)]
pub(super) struct ThreadNameSuggestionJob {
    pub(super) parent_thread_id: ThreadId,
    pub(super) kind: ThreadNameSuggestionKind,
    pub(super) current_name: Option<String>,
    pub(super) latest_final_answer: Option<String>,
    pub(super) completion_queued: bool,
}

impl App {
    fn thread_name_suggestion_developer_instructions(
        existing_instructions: Option<&str>,
    ) -> String {
        match existing_instructions {
            Some(existing_instructions) if !existing_instructions.trim().is_empty() => {
                format!(
                    "{existing_instructions}\n\n{THREAD_NAME_SUGGESTION_DEVELOPER_INSTRUCTIONS}"
                )
            }
            _ => THREAD_NAME_SUGGESTION_DEVELOPER_INSTRUCTIONS.to_string(),
        }
    }

    fn thread_name_suggestion_fork_config(&self) -> Config {
        let mut fork_config = self.config.clone();
        fork_config.ephemeral = true;
        fork_config.developer_instructions =
            Some(Self::thread_name_suggestion_developer_instructions(
                fork_config.developer_instructions.as_deref(),
            ));
        fork_config
    }

    pub(super) async fn start_thread_name_suggestion(
        &mut self,
        app_server: &mut AppServerSession,
        parent_thread_id: ThreadId,
        kind: ThreadNameSuggestionKind,
        prompt: String,
        current_name: Option<String>,
    ) {
        self.refresh_in_memory_config_from_disk_best_effort("generating a thread title")
            .await;
        let fork_config = self.thread_name_suggestion_fork_config();
        let forked = match app_server.fork_thread(fork_config, parent_thread_id).await {
            Ok(forked) => forked,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to generate thread title: {err}"));
                return;
            }
        };

        let child_session = forked.session;
        let child_thread_id = child_session.thread_id;
        self.thread_name_suggestion_jobs.insert(
            child_thread_id,
            ThreadNameSuggestionJob {
                parent_thread_id,
                kind,
                current_name,
                latest_final_answer: None,
                completion_queued: false,
            },
        );

        let result = app_server
            .turn_start(
                child_thread_id,
                vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                child_session.cwd.to_path_buf(),
                child_session.approval_policy,
                child_session.approvals_reviewer,
                child_session.permission_profile,
                child_session.active_permission_profile,
                child_session.model,
                child_session.reasoning_effort,
                /*summary*/ None,
                /*service_tier*/ None,
                /*collaboration_mode*/ None,
                /*personality*/ None,
                /*final_output_json_schema*/ None,
            )
            .await;
        if let Err(err) = result {
            self.thread_name_suggestion_jobs.remove(&child_thread_id);
            self.chat_widget
                .add_error_message(format!("Failed to generate thread title: {err}"));
            if let Err(cleanup_err) = app_server.thread_unsubscribe(child_thread_id).await {
                tracing::warn!(
                    thread_id = %child_thread_id,
                    error = %cleanup_err,
                    "failed to unsubscribe failed thread-title suggestion worker"
                );
            }
        }
    }

    pub(super) fn consume_thread_name_suggestion_notification(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) -> bool {
        let Some(job) = self.thread_name_suggestion_jobs.get_mut(&thread_id) else {
            return false;
        };
        if job.completion_queued {
            return true;
        }

        if let Some(suggestion) = suggestion_from_notification(notification) {
            job.latest_final_answer = Some(suggestion.to_string());
            return true;
        }

        let ServerNotification::TurnCompleted(notification) = notification else {
            return true;
        };
        job.completion_queued = true;
        let result = match notification.turn.status {
            TurnStatus::Completed => {
                completed_suggestion(job.latest_final_answer.as_deref(), &notification.turn)
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| "title generator completed without a final answer".to_string())
            }
            TurnStatus::Interrupted => Err("title generator was interrupted".to_string()),
            TurnStatus::Failed => Err(notification
                .turn
                .error
                .as_ref()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "title generator failed".to_string())),
            TurnStatus::InProgress => {
                job.completion_queued = false;
                return true;
            }
        };
        self.app_event_tx
            .send(AppEvent::ThreadNameSuggestionFinished {
                child_thread_id: thread_id,
                result,
            });
        true
    }

    pub(super) fn consume_thread_name_suggestion_request(&mut self, thread_id: ThreadId) -> bool {
        let Some(job) = self.thread_name_suggestion_jobs.get_mut(&thread_id) else {
            return false;
        };
        if !job.completion_queued {
            job.completion_queued = true;
            self.app_event_tx
                .send(AppEvent::ThreadNameSuggestionFinished {
                child_thread_id: thread_id,
                result: Err(
                    "title generator requested an interactive action instead of returning a title"
                        .to_string(),
                ),
            });
        }
        true
    }

    pub(super) async fn finish_thread_name_suggestion(
        &mut self,
        app_server: &mut AppServerSession,
        child_thread_id: ThreadId,
        result: Result<String, String>,
    ) {
        let Some(job) = self.thread_name_suggestion_jobs.remove(&child_thread_id) else {
            return;
        };
        let failed = result.is_err();

        match result {
            Ok(suggestion) => {
                match compose_thread_name(job.kind, job.current_name.as_deref(), &suggestion) {
                    Some(name) => {
                        if let Err(err) =
                            app_server.thread_set_name(job.parent_thread_id, name).await
                        {
                            self.chat_widget
                                .add_error_message(format!("Failed to update thread title: {err}"));
                        }
                    }
                    None => self.chat_widget.add_error_message(
                        "Generated thread title was empty; leaving the current title unchanged."
                            .to_string(),
                    ),
                }
            }
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to generate thread title: {err}")),
        }

        if failed && let Err(err) = app_server.startup_interrupt(child_thread_id).await {
            tracing::warn!(
                thread_id = %child_thread_id,
                error = %err,
                "failed to interrupt thread-title suggestion worker after failure"
            );
        }
        if let Err(err) = app_server.thread_unsubscribe(child_thread_id).await {
            tracing::warn!(
                thread_id = %child_thread_id,
                error = %err,
                "failed to unsubscribe thread-title suggestion worker"
            );
        }
    }
}

fn suggestion_from_turn(turn: &Turn) -> Option<&str> {
    turn.items.iter().rev().find_map(|item| match item {
        ThreadItem::AgentMessage { text, phase, .. }
            if matches!(phase, Some(MessagePhase::FinalAnswer) | None) =>
        {
            Some(text.as_str())
        }
        _ => None,
    })
}

fn suggestion_from_notification(notification: &ServerNotification) -> Option<&str> {
    let ServerNotification::ItemCompleted(notification) = notification else {
        return None;
    };

    match &notification.item {
        ThreadItem::AgentMessage { text, phase, .. }
            if matches!(phase, Some(MessagePhase::FinalAnswer) | None) =>
        {
            Some(text.as_str())
        }
        _ => None,
    }
}

fn completed_suggestion<'a>(
    latest_final_answer: Option<&'a str>,
    turn: &'a Turn,
) -> Option<&'a str> {
    latest_final_answer.or_else(|| suggestion_from_turn(turn))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ItemCompletedNotification;
    use pretty_assertions::assert_eq;

    #[test]
    fn suggestion_from_turn_prefers_final_agent_message() {
        let turn = Turn {
            id: "turn-1".to_string(),
            items: vec![
                ThreadItem::AgentMessage {
                    id: "commentary".to_string(),
                    text: "working".to_string(),
                    phase: Some(MessagePhase::Commentary),
                    memory_citation: None,
                },
                ThreadItem::AgentMessage {
                    id: "final".to_string(),
                    text: "🧭 Session title".to_string(),
                    phase: Some(MessagePhase::FinalAnswer),
                    memory_citation: None,
                },
            ],
            status: TurnStatus::Completed,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };

        assert_eq!(suggestion_from_turn(&turn), Some("🧭 Session title"));
    }

    #[test]
    fn suggestion_from_notification_reads_completed_final_agent_message() {
        let notification = ServerNotification::ItemCompleted(ItemCompletedNotification {
            item: ThreadItem::AgentMessage {
                id: "final".to_string(),
                text: "🧭 Session title".to_string(),
                phase: Some(MessagePhase::FinalAnswer),
                memory_citation: None,
            },
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 0,
        });

        assert_eq!(
            suggestion_from_notification(&notification),
            Some("🧭 Session title")
        );
    }

    #[test]
    fn completed_suggestion_uses_streamed_final_answer_when_completion_has_no_items() {
        let turn = Turn {
            id: "turn-1".to_string(),
            items: Vec::new(),
            status: TurnStatus::Completed,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };

        assert_eq!(
            completed_suggestion(Some("🧭 Session title"), &turn),
            Some("🧭 Session title")
        );
    }
}
