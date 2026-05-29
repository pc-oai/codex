use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadCloseParams;
use codex_app_server_protocol::ThreadCloseResponse;
use codex_app_server_protocol::ThreadSpawnHistory;
use codex_app_server_protocol::ThreadSpawnParams;
use codex_app_server_protocol::ThreadSpawnResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
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
            history: None,
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
    let mut started_response_thread = child.clone();
    started_response_thread.turns.clear();
    assert_eq!(started.thread, started_response_thread);

    Ok(())
}

#[tokio::test]
async fn thread_spawn_without_input_creates_idle_child() -> Result<()> {
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
    let parent_turn_request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: parent.id.clone(),
            input: vec![UserInput::Text {
                text: "Seed parent context.".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let parent_turn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(parent_turn_request_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response::<TurnStartResponse>(parent_turn_response)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let spawn_request_id = mcp
        .send_thread_spawn_request(ThreadSpawnParams {
            thread_id: parent.id.clone(),
            task_name: Some("idle_child".to_string()),
            history: None,
            input: Vec::new(),
        })
        .await?;
    let spawn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(spawn_request_id)),
    )
    .await??;
    let child = to_response::<ThreadSpawnResponse>(spawn_response)?.thread;

    assert_eq!(child.status, ThreadStatus::Idle);
    assert_eq!(child.forked_from_id, Some(parent.id.clone()));
    assert_eq!(child.turns.len(), 1, "expected copied parent history");
    assert!(matches!(
        child.source,
        codex_app_server_protocol::SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            agent_path: Some(ref agent_path),
            ..
        }) if agent_path.as_str() == "/root/idle_child"
    ));

    let fresh_spawn_request_id = mcp
        .send_thread_spawn_request(ThreadSpawnParams {
            thread_id: parent.id.clone(),
            task_name: Some("fresh_child".to_string()),
            history: Some(ThreadSpawnHistory::Fresh),
            input: Vec::new(),
        })
        .await?;
    let fresh_spawn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(fresh_spawn_request_id)),
    )
    .await??;
    let fresh_child = to_response::<ThreadSpawnResponse>(fresh_spawn_response)?.thread;

    assert_eq!(fresh_child.status, ThreadStatus::Idle);
    assert_eq!(fresh_child.forked_from_id, None);
    assert!(fresh_child.turns.is_empty(), "expected fresh child history");

    let close_request_id = mcp
        .send_thread_close_request(ThreadCloseParams {
            thread_id: fresh_child.id,
        })
        .await?;
    let close_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(close_request_id)),
    )
    .await??;
    let _: ThreadCloseResponse = to_response::<ThreadCloseResponse>(close_response)?;

    Ok(())
}
