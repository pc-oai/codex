//! User-triggered child-agent creation from the active TUI thread.

use super::*;

impl App {
    pub(super) async fn handle_start_subagent(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        parent_thread_id: ThreadId,
        prompt: Option<String>,
        switch_to_child: bool,
    ) {
        match app_server
            .spawn_subagent(&self.config, parent_thread_id, prompt)
            .await
        {
            Ok(spawned) => {
                let started = spawned.started;
                let thread_id = started.session.thread_id;
                let agent_nickname = spawned.agent_nickname;
                let agent_role = spawned.agent_role;
                let channel = self.ensure_thread_channel(thread_id);
                {
                    let mut store = channel.store.lock().await;
                    store.set_session(started.session, started.turns);
                }
                self.upsert_agent_picker_thread(
                    thread_id,
                    agent_nickname.clone(),
                    agent_role.clone(),
                    /*is_closed*/ false,
                );
                if switch_to_child {
                    if let Err(err) = self
                        .select_agent_thread_and_discard_side(tui, app_server, thread_id)
                        .await
                    {
                        self.chat_widget.add_error_message(format!(
                            "Failed to open subagent {thread_id}: {err}"
                        ));
                    }
                    return;
                }
                let label = format_agent_picker_item_name(
                    agent_nickname.as_deref(),
                    agent_role.as_deref(),
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
