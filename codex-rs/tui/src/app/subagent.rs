//! User-triggered child-agent creation from the active TUI thread.

use super::*;

impl App {
    pub(super) async fn close_active_subagent_from_shortcut(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        switch_target_thread_id: ThreadId,
    ) {
        if let Err(err) = app_server.thread_close(thread_id).await {
            self.chat_widget
                .add_error_message(format!("Failed to close subagent {thread_id}: {err}"));
            return;
        }

        self.mark_agent_picker_thread_closed(thread_id);
        if let Err(err) = self
            .select_agent_thread_and_discard_side(tui, app_server, switch_target_thread_id)
            .await
        {
            self.chat_widget.add_error_message(format!(
                "Closed subagent {thread_id}, but failed to open agent thread {switch_target_thread_id}: {err}"
            ));
            return;
        }
        if self.active_thread_id == Some(switch_target_thread_id) {
            self.chat_widget
                .add_info_message(format!("Closed subagent {thread_id}."), /*hint*/ None);
        }
    }

    pub(super) async fn handle_start_subagent(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        parent_thread_id: ThreadId,
        prompt: Option<String>,
        history: ThreadSpawnHistory,
        switch_to_child: bool,
    ) {
        self.chat_widget.set_subagent_spawn_pending(true);
        let terminal_resize_reflow_enabled = self.terminal_resize_reflow_enabled();
        if terminal_resize_reflow_enabled && let Err(err) = self.handle_draw_pre_render(tui) {
            tracing::warn!(error = %err, "failed to prepare subagent start status");
        }
        self.chat_widget.pre_draw_tick();
        if let Err(err) = self.render_chat_widget_frame(tui, terminal_resize_reflow_enabled) {
            tracing::warn!(error = %err, "failed to render subagent start status");
        }

        let spawn_result = app_server
            .spawn_subagent(&self.config, parent_thread_id, prompt, history)
            .await;
        self.chat_widget.set_subagent_spawn_pending(false);

        match spawn_result {
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
