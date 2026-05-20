//! User-triggered child-agent creation from the active TUI thread.

use super::*;

impl App {
    pub(super) async fn handle_start_subagent(
        &mut self,
        app_server: &mut AppServerSession,
        parent_thread_id: ThreadId,
        prompt: String,
    ) {
        match app_server.spawn_subagent(parent_thread_id, prompt).await {
            Ok(thread) => {
                if let Ok(thread_id) = ThreadId::from_string(&thread.id) {
                    self.upsert_agent_picker_thread(
                        thread_id,
                        thread.agent_nickname.clone(),
                        thread.agent_role.clone(),
                        /*is_closed*/ false,
                    );
                }
                let label = format_agent_picker_item_name(
                    thread.agent_nickname.as_deref(),
                    thread.agent_role.as_deref(),
                    /*is_primary*/ false,
                );
                self.chat_widget.add_info_message(
                    format!("Started {label}."),
                    Some("Use /agent to switch to it.".to_string()),
                );
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to start subagent: {err}"));
            }
        }
    }
}
