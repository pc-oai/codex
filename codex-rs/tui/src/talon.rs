use std::fs;
use std::io;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::openai_models::ReasoningEffort;
use codex_uds::UnixListener;
use codex_uds::UnixStream;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::error;
use tracing::info;
use tracing::warn;

const TALON_DIR_NAME: &str = ".codex-talon";
const STATE_FILENAME: &str = "state.json";
const SOCKET_FILENAME: &str = "command.sock";

#[cfg(unix)]
const TALON_SOCKET_MODE: u32 = 0o600;

static STATUS_SUMMARY: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub(crate) struct TalonPaths {
    pub state_path: PathBuf,
    pub socket_path: PathBuf,
}

pub(crate) fn resolve_session_paths(session_id: &str) -> Result<TalonPaths> {
    let home = dirs::home_dir().context("unable to locate home directory for Talon RPC paths")?;
    resolve_paths_in_dir(home.join(TALON_DIR_NAME).join(session_id))
}

fn resolve_paths_in_dir(base_dir: PathBuf) -> Result<TalonPaths> {
    if !base_dir.exists() {
        fs::create_dir_all(&base_dir).context("failed to create Talon RPC directory")?;
    }

    let state_path = base_dir.join(STATE_FILENAME);
    let socket_path = base_dir.join(SOCKET_FILENAME);

    Ok(TalonPaths {
        state_path,
        socket_path,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct TalonRequest {
    #[serde(default)]
    pub commands: Vec<TalonCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum TalonCommand {
    /// Replace the entire composer buffer with `text`. Optionally update the cursor.
    SetBuffer {
        text: String,
        #[serde(default)]
        cursor: Option<usize>,
    },
    /// Move the cursor to the provided absolute byte offset within the buffer.
    SetCursor { cursor: usize },
    /// No-op request that asks Codex to write its current state snapshot.
    GetState,
    /// Post a lightweight notification (no buffer/cursor change).
    Notify { message: String },
    /// Trigger editing of a previous composer-history entry.
    EditPreviousMessage {
        #[serde(default)]
        steps_back: usize,
    },
    /// Rewind to the latest user message and prefill it for editing.
    EditLastMessage,
    /// Copy the latest visible user request.
    CopyLastRequest,
    /// Copy the latest completed agent response.
    CopyLastResponse,
    /// Generate a fresh title suggestion for the current thread.
    RetitleCurrentSession,
    /// Rename the current thread to an explicit user-provided title.
    RenameCurrentSession { name: String },
    /// Generate or refresh the leading emoji for the current thread title.
    EmojiCurrentSession,
    /// Interrupt the currently running turn, if any.
    InterruptCurrentTurn,
    /// Exit Codex after graceful shutdown.
    ExitCurrentSession,
    /// Select a model and reasoning effort for future turns.
    SetModel {
        model: String,
        #[serde(default)]
        effort: Option<ReasoningEffort>,
    },
    /// Restart Codex only when no turn is currently running.
    ReloadCurrentSessionIfIdle,
    /// Restart Codex and resume the current session.
    ReloadCurrentSession,
    /// Navigate to the previous entry in the composer history.
    HistoryPrevious,
    /// Navigate to the next entry in the composer history.
    HistoryNext,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TalonResponseStatus {
    Ok,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonEditorState {
    pub buffer: String,
    pub cursor: usize,
    pub is_task_running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_user_request: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_user_requests: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_agent_responses: Vec<String>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_build_number: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonModelState {
    pub model: String,
    pub display_name: String,
    pub default_reasoning_effort: ReasoningEffort,
    pub supported_reasoning_efforts: Vec<ReasoningEffort>,
    pub show_in_picker: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonAmbientState {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_build_number: Option<u32>,
    pub is_task_running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_user_request: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_user_requests: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_agent_responses: Vec<String>,
    pub current_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_reasoning_effort: Option<ReasoningEffort>,
    pub models: Vec<TalonModelState>,
    pub timestamp_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonResponse {
    pub version: u32,
    pub status: TalonResponseStatus,
    pub state: TalonEditorState,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub applied: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timestamp_ms: u128,
}

pub(crate) struct TalonSocketRequest {
    pub request: TalonRequest,
    pub response_tx: oneshot::Sender<TalonResponse>,
}

pub(crate) async fn start_socket_acceptor(
    paths: &TalonPaths,
) -> Result<(mpsc::UnboundedReceiver<TalonSocketRequest>, JoinHandle<()>)> {
    prepare_socket_path(&paths.socket_path)
        .await
        .with_context(|| {
            format!(
                "failed to prepare Talon socket path {}",
                paths.socket_path.display()
            )
        })?;
    let listener = UnixListener::bind(&paths.socket_path)
        .await
        .with_context(|| {
            format!(
                "failed to bind Talon socket path {}",
                paths.socket_path.display()
            )
        })?;
    set_socket_permissions(&paths.socket_path)
        .await
        .with_context(|| {
            format!(
                "failed to set Talon socket permissions {}",
                paths.socket_path.display()
            )
        })?;

    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let socket_guard = TalonSocketFileGuard {
        socket_path: paths.socket_path.clone(),
    };
    info!(
        socket_path = %paths.socket_path.display(),
        "Talon command socket listening"
    );
    let join_handle = tokio::spawn(run_socket_acceptor(listener, request_tx, socket_guard));

    Ok((request_rx, join_handle))
}

/// Returns true while a live TUI still owns the per-session command socket.
///
/// The resume picker uses this to tell open local sessions apart from saved
/// sessions without creating any Talon state directories as a side effect.
pub(crate) async fn session_command_socket_is_live(session_id: &str) -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let socket_path = home
        .join(TALON_DIR_NAME)
        .join(session_id)
        .join(SOCKET_FILENAME);
    socket_path_has_live_listener(&socket_path).await
}

async fn socket_path_has_live_listener(socket_path: &Path) -> bool {
    UnixStream::connect(socket_path).await.is_ok()
}

async fn run_socket_acceptor(
    mut listener: UnixListener,
    request_tx: mpsc::UnboundedSender<TalonSocketRequest>,
    socket_guard: TalonSocketFileGuard,
) {
    let _socket_guard = socket_guard;
    loop {
        let stream = match listener.accept().await {
            Ok(stream) => stream,
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::ConnectionAborted
                        | ErrorKind::ConnectionReset
                        | ErrorKind::Interrupted
                ) =>
            {
                warn!("recoverable Talon command socket accept error: {err}");
                continue;
            }
            Err(err) => {
                error!("Talon command socket accept error: {err}");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        let request_tx = request_tx.clone();
        tokio::spawn(async move {
            if let Err(err) = serve_socket_connection(stream, request_tx).await {
                warn!("Talon command socket connection failed: {err}");
            }
        });
    }
}

async fn serve_socket_connection(
    mut stream: UnixStream,
    request_tx: mpsc::UnboundedSender<TalonSocketRequest>,
) -> Result<()> {
    let mut payload = Vec::new();
    stream
        .read_to_end(&mut payload)
        .await
        .context("failed to read Talon socket request")?;
    let request: TalonRequest =
        serde_json::from_slice(&payload).context("failed to parse Talon socket request")?;

    let (response_tx, response_rx) = oneshot::channel();
    request_tx
        .send(TalonSocketRequest {
            request,
            response_tx,
        })
        .map_err(|_| anyhow::anyhow!("Talon socket request receiver is closed"))?;

    let response = response_rx
        .await
        .context("Talon socket response channel closed")?;
    let serialized =
        serde_json::to_vec(&response).context("failed to serialize Talon socket response")?;
    stream
        .write_all(&serialized)
        .await
        .context("failed to write Talon socket response")?;
    stream
        .shutdown()
        .await
        .context("failed to shut down Talon socket response stream")?;
    Ok(())
}

async fn prepare_socket_path(socket_path: &Path) -> io::Result<()> {
    if let Some(parent) = socket_path.parent() {
        codex_uds::prepare_private_socket_directory(parent).await?;
    }

    match UnixStream::connect(socket_path).await {
        Ok(_stream) => {
            return Err(io::Error::new(
                ErrorKind::AddrInUse,
                format!(
                    "Talon command socket is already in use at {}",
                    socket_path.display()
                ),
            ));
        }
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) if err.kind() == ErrorKind::ConnectionRefused => {}
        Err(err) => {
            if !socket_path.exists() {
                return Ok(());
            }
            return Err(err);
        }
    }

    if !socket_path.try_exists()? {
        return Ok(());
    }

    if !codex_uds::is_stale_socket_path(socket_path).await? {
        return Err(io::Error::new(
            ErrorKind::AlreadyExists,
            format!(
                "Talon command socket path exists and is not a socket: {}",
                socket_path.display()
            ),
        ));
    }
    tokio::fs::remove_file(socket_path).await
}

#[cfg(unix)]
async fn set_socket_permissions(socket_path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    tokio::fs::set_permissions(
        socket_path,
        std::fs::Permissions::from_mode(TALON_SOCKET_MODE),
    )
    .await
}

#[cfg(not(unix))]
async fn set_socket_permissions(_socket_path: &Path) -> io::Result<()> {
    Ok(())
}

struct TalonSocketFileGuard {
    socket_path: PathBuf,
}

impl Drop for TalonSocketFileGuard {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.socket_path)
            && err.kind() != ErrorKind::NotFound
        {
            warn!(
                socket_path = %self.socket_path.display(),
                %err,
                "failed to remove Talon command socket file"
            );
        }
    }
}

pub(crate) fn write_state(paths: &TalonPaths, state: &TalonAmbientState) -> Result<()> {
    let payload = serde_json::to_vec_pretty(state).context("failed to serialize Talon state")?;
    fs::write(&paths.state_path, payload).with_context(|| {
        format!(
            "failed to write Talon state to {}",
            paths.state_path.display()
        )
    })
}

pub(crate) fn now_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

pub(crate) fn set_status_summary(summary: Option<String>) {
    if let Ok(mut guard) = STATUS_SUMMARY.lock() {
        *guard = summary;
    }
}

pub(crate) fn status_summary() -> Option<String> {
    STATUS_SUMMARY.lock().ok().and_then(|guard| guard.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::Value;

    #[tokio::test]
    async fn socket_acceptor_round_trips_talon_requests() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let paths =
            resolve_paths_in_dir(tempdir.path().join("session")).expect("resolve socket paths");
        let (mut request_rx, acceptor) = start_socket_acceptor(&paths)
            .await
            .expect("start Talon command socket");

        let responder = tokio::spawn(async move {
            let socket_request = request_rx.recv().await.expect("socket request");
            assert!(matches!(
                socket_request.request.commands.as_slice(),
                [TalonCommand::GetState]
            ));
            let response = TalonResponse {
                version: 1,
                status: TalonResponseStatus::Ok,
                state: TalonEditorState {
                    buffer: "hello".to_string(),
                    cursor: 5,
                    is_task_running: false,
                    task_summary: None,
                    session_id: Some("session".to_string()),
                    cwd: None,
                    last_user_request: None,
                    recent_user_requests: Vec::new(),
                    recent_agent_responses: Vec::new(),
                    model: "gengar-dex".to_string(),
                    reasoning_effort: None,
                    local_build_number: Some(13),
                },
                applied: vec!["get_state".to_string()],
                error: None,
                timestamp_ms: 42,
            };
            socket_request
                .response_tx
                .send(response)
                .expect("send socket response");
        });

        let mut stream = UnixStream::connect(&paths.socket_path)
            .await
            .expect("connect Talon command socket");
        stream
            .write_all(br#"{"commands":[{"type":"get_state"}]}"#)
            .await
            .expect("write request");
        stream.shutdown().await.expect("close request half");
        let mut payload = Vec::new();
        stream
            .read_to_end(&mut payload)
            .await
            .expect("read response");
        let response: Value = serde_json::from_slice(&payload).expect("parse response");
        assert_eq!(response["state"]["buffer"], "hello");
        assert_eq!(response["state"]["cursor"], 5);
        assert_eq!(response["applied"][0], "get_state");

        responder.await.expect("responder task");
        acceptor.abort();
    }

    #[tokio::test]
    async fn live_socket_probe_distinguishes_listeners_from_missing_paths() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let paths =
            resolve_paths_in_dir(tempdir.path().join("session")).expect("resolve socket paths");

        assert!(!socket_path_has_live_listener(&paths.socket_path).await);

        let (_request_rx, acceptor) = start_socket_acceptor(&paths)
            .await
            .expect("start Talon command socket");

        assert!(socket_path_has_live_listener(&paths.socket_path).await);

        acceptor.abort();
    }
}
