use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadSpawnParams;
use codex_app_server_protocol::ThreadSpawnResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::UserInput;
use codex_protocol::protocol::SubAgentSource;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_spawn_creates_child_in_parent_session_tree() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::default(),
        i64::MAX,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "",
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let parent_request_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let parent_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(parent_request_id)),
    )
    .await??;
    let parent = to_response::<ThreadStartResponse>(parent_response)?.thread;

    let spawn_request_id = mcp
        .send_thread_spawn_request(ThreadSpawnParams {
            thread_id: parent.id.clone(),
            task_name: Some("parser_check".to_string()),
            input: vec![UserInput::Text {
                text: "Check the parser tests.".to_string(),
                text_elements: Vec::new(),
            }],
        })
        .await?;
    let spawn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(spawn_request_id)),
    )
    .await??;
    let child = to_response::<ThreadSpawnResponse>(spawn_response)?.thread;

    assert_eq!(child.session_id, parent.session_id);
    assert_eq!(
        child.thread_source,
        Some(codex_app_server_protocol::ThreadSource::Subagent)
    );
    assert!(matches!(
        child.source,
        codex_app_server_protocol::SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            agent_path: Some(ref agent_path),
            ..
        }) if parent_thread_id.to_string() == parent.id
            && agent_path.as_str() == "/root/parser_check"
    ));

    let deadline = tokio::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    let started = loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = timeout(remaining, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        if notification.method != "thread/started" {
            continue;
        }
        let started: ThreadStartedNotification =
            serde_json::from_value(notification.params.expect("thread/started params"))?;
        if started.thread.id == child.id {
            break started;
        }
    };
    assert_eq!(started.thread, child);

    Ok(())
}
