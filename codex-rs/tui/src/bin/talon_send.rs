use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use dirs::home_dir;
use serde::Serialize;
use serde_json::Value;

const TALON_DIR: &str = ".codex-talon";
const REQUEST_FILE: &str = "request.json";
const RESPONSE_FILE: &str = "response.json";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Send commands to the Codex Talon command server"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Replace the Codex input buffer (optional cursor).
    SetBuffer {
        /// Text to populate the buffer with.
        #[arg(short, long)]
        text: String,
        /// Optional cursor offset within the new buffer.
        #[arg(short, long)]
        cursor: Option<usize>,
    },
    /// Move cursor to an absolute byte offset within the buffer.
    SetCursor {
        /// Cursor position to set.
        cursor: usize,
    },
    /// Clear any pending request file.
    Clear,
    /// Stage a request for Codex to emit its current state.
    State,
    /// Print the most recent response/state file.
    ShowState {
        /// Emit raw JSON without pretty formatting.
        #[arg(long)]
        raw: bool,
    },
    /// Stage a flash notification inside Codex.
    Notify {
        /// Text to display.
        message: String,
    },
    /// Navigate to the previous entry in the composer history.
    HistoryPrevious,
    /// Navigate to the next entry in the composer history.
    HistoryNext,
    /// Prefill the composer by stepping back N entries in history.
    EditPrevious {
        /// Number of entries to step back from the latest.
        #[arg(default_value_t = 0)]
        steps_back: usize,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct TalonRequest {
    commands: Vec<TalonCommand>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TalonCommand {
    SetBuffer {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cursor: Option<usize>,
    },
    SetCursor {
        cursor: usize,
    },
    GetState,
    Notify {
        message: String,
    },
    EditPreviousMessage {
        #[serde(default)]
        steps_back: usize,
    },
    HistoryPrevious,
    HistoryNext,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (request_path, response_path) = ensure_paths()?;

    let message = match cli.command {
        Command::SetBuffer { text, cursor } => {
            let request = TalonRequest {
                commands: vec![TalonCommand::SetBuffer { text, cursor }],
            };
            write_request(&request_path, request)?;
            format!("wrote request to {}", request_path.display())
        }
        Command::SetCursor { cursor } => {
            let request = TalonRequest {
                commands: vec![TalonCommand::SetCursor { cursor }],
            };
            write_request(&request_path, request)?;
            format!("wrote request to {}", request_path.display())
        }
        Command::Clear => {
            if let Err(err) = fs::remove_file(&request_path)
                && err.kind() != std::io::ErrorKind::NotFound
            {
                return Err(err.into());
            }
            format!("cleared request at {}", request_path.display())
        }
        Command::State => {
            let request = TalonRequest {
                commands: vec![TalonCommand::GetState],
            };
            write_request(&request_path, request)?;
            format!("requested state via {}", request_path.display())
        }
        Command::Notify { message } => {
            let request = TalonRequest {
                commands: vec![TalonCommand::Notify { message }],
            };
            write_request(&request_path, request)?;
            format!("requested notification via {}", request_path.display())
        }
        Command::HistoryPrevious => {
            let request = TalonRequest {
                commands: vec![TalonCommand::HistoryPrevious],
            };
            write_request(&request_path, request)?;
            format!("requested history_previous via {}", request_path.display())
        }
        Command::HistoryNext => {
            let request = TalonRequest {
                commands: vec![TalonCommand::HistoryNext],
            };
            write_request(&request_path, request)?;
            format!("requested history_next via {}", request_path.display())
        }
        Command::EditPrevious { steps_back } => {
            let request = TalonRequest {
                commands: vec![TalonCommand::EditPreviousMessage { steps_back }],
            };
            write_request(&request_path, request)?;
            format!(
                "requested edit_previous_message({steps_back}) via {}",
                request_path.display()
            )
        }
        Command::ShowState { raw } => {
            print_state(&response_path, raw)?;
            return Ok(());
        }
    };

    println!("{message}");
    Ok(())
}

fn ensure_paths() -> Result<(PathBuf, PathBuf)> {
    let home = home_dir().context("unable to locate home directory")?;
    let dir = home.join(TALON_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    }
    Ok((dir.join(REQUEST_FILE), dir.join(RESPONSE_FILE)))
}

fn write_request(path: &PathBuf, request: TalonRequest) -> Result<()> {
    let payload =
        serde_json::to_vec_pretty(&request).context("failed to serialize Talon request")?;
    fs::write(path, payload).with_context(|| format!("failed to write {}", path.display()))
}

fn print_state(path: &PathBuf, raw: bool) -> Result<()> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    if raw {
        println!("{contents}");
        return Ok(());
    }

    let value: Value = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse JSON from {}", path.display()))?;
    let pretty = serde_json::to_string_pretty(&value)?;
    println!("{pretty}");
    Ok(())
}
