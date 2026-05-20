use strum::IntoEnumIterator;
use strum_macros::AsRefStr;
use strum_macros::EnumIter;
use strum_macros::EnumString;
use strum_macros::IntoStaticStr;

/// Commands that can be invoked by starting a message with a leading slash.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, AsRefStr, IntoStaticStr,
)]
#[strum(serialize_all = "kebab-case")]
pub enum SlashCommand {
    // DO NOT ALPHA-SORT! Enum order is presentation order in the popup, so
    // more frequently used commands should be listed first.
    #[strum(to_string = "model", serialize = "m")]
    Model,
    #[strum(to_string = "effort", serialize = "e")]
    Effort,
    Fast,
    Ide,
    Permissions,
    Keymap,
    Vim,
    #[strum(serialize = "setup-default-sandbox")]
    ElevateSandbox,
    #[strum(serialize = "sandbox-add-read-dir")]
    SandboxReadRoot,
    Experimental,
    #[strum(to_string = "approve")]
    AutoReview,
    Memories,
    Skills,
    Hooks,
    #[strum(to_string = "review", serialize = "rev")]
    Review,
    Rename,
    Park,
    Done,
    #[strum(to_string = "active", serialize = "reopen")]
    Active,
    #[strum(to_string = "retitle", serialize = "rt")]
    Retitle,
    #[strum(to_string = "emoji", serialize = "em")]
    Emoji,
    New,
    Resume,
    #[strum(to_string = "reload", serialize = "r")]
    Reload,
    Fork,
    Init,
    #[strum(to_string = "compact", serialize = "c")]
    Compact,
    Condensed,
    Plan,
    Goal,
    Collab,
    Agent,
    Side,
    #[strum(to_string = "id", serialize = "i")]
    Id,
    Copy,
    CopyLastRequest,
    Diff,
    Mention,
    Status,
    DebugConfig,
    #[strum(to_string = "title", serialize = "t")]
    Title,
    Statusline,
    Theme,
    Mcp,
    Apps,
    Plugins,
    Logout,
    Quit,
    Exit,
    Delete,
    Feedback,
    Rollout,
    Ps,
    #[strum(to_string = "stop", serialize = "clean")]
    Stop,
    Clear,
    Personality,
    Realtime,
    Settings,
    TestApproval,
    #[strum(serialize = "subagents")]
    MultiAgents,
    // Debugging commands.
    #[strum(serialize = "debug-m-drop")]
    MemoryDrop,
    #[strum(serialize = "debug-m-update")]
    MemoryUpdate,
}

impl SlashCommand {
    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        match self {
            SlashCommand::Feedback => "send logs to maintainers",
            SlashCommand::New => "start a new chat during a conversation",
            SlashCommand::Init => "create an AGENTS.md file with instructions for Codex",
            SlashCommand::Compact => "summarize conversation to prevent hitting the context limit",
            SlashCommand::Condensed => {
                "toggle message-only main transcript view in terminal scrollback"
            }
            SlashCommand::Review => "review my current changes and find issues",
            SlashCommand::Rename => "rename the current thread",
            SlashCommand::Park => "mark the current thread as parked",
            SlashCommand::Done => "mark the current thread as done",
            SlashCommand::Active => "mark the current thread as active",
            SlashCommand::Retitle => "generate a concise title from this conversation",
            SlashCommand::Emoji => "prepend a representative emoji to the thread title",
            SlashCommand::Resume => "resume a saved chat",
            SlashCommand::Reload => "restart Codex and resume this chat",
            SlashCommand::Clear => "clear the terminal and start a new chat",
            SlashCommand::Fork => "fork the current chat",
            SlashCommand::Quit | SlashCommand::Exit => "exit Codex",
            SlashCommand::Delete => "delete this chat and exit Codex",
            SlashCommand::Id => "copy the current thread ID",
            SlashCommand::Copy => "copy last response as markdown",
            SlashCommand::CopyLastRequest => "copy last user request",
            SlashCommand::Diff => "show git diff (including untracked files)",
            SlashCommand::Mention => "mention a file",
            SlashCommand::Skills => "use skills to improve how Codex performs specific tasks",
            SlashCommand::Hooks => "view and manage lifecycle hooks",
            SlashCommand::Status => "show current session configuration and token usage",
            SlashCommand::DebugConfig => "show config layers and requirement sources for debugging",
            SlashCommand::Title => "configure which items appear in the terminal title",
            SlashCommand::Statusline => "configure which items appear in the status line",
            SlashCommand::Theme => "choose a syntax highlighting theme",
            SlashCommand::Ps => "list background terminals",
            SlashCommand::Stop => "stop all background terminals",
            SlashCommand::MemoryDrop => "DO NOT USE",
            SlashCommand::MemoryUpdate => "DO NOT USE",
            SlashCommand::Model => "choose what model and reasoning effort to use",
            SlashCommand::Effort => "choose reasoning effort for the current model",
            SlashCommand::Fast => {
                "toggle Fast mode to enable fastest inference with increased plan usage"
            }
            SlashCommand::Ide => {
                "include current selection, open files, and other context from your IDE"
            }
            SlashCommand::Personality => "choose a communication style for Codex",
            SlashCommand::Realtime => "toggle realtime voice mode (experimental)",
            SlashCommand::Settings => "configure realtime microphone/speaker",
            SlashCommand::Plan => "switch to Plan mode",
            SlashCommand::Goal => "set or view the goal for a long-running task",
            SlashCommand::Collab => "change collaboration mode (experimental)",
            SlashCommand::Agent | SlashCommand::MultiAgents => "switch the active agent thread",
            SlashCommand::Side => "start a side conversation in an ephemeral fork",
            SlashCommand::Permissions => "choose what Codex is allowed to do",
            SlashCommand::Keymap => "remap TUI shortcuts",
            SlashCommand::Vim => "toggle Vim mode for the composer",
            SlashCommand::ElevateSandbox => "set up elevated agent sandbox",
            SlashCommand::SandboxReadRoot => {
                "let sandbox read a directory: /sandbox-add-read-dir <absolute_path>"
            }
            SlashCommand::Experimental => "toggle experimental features",
            SlashCommand::AutoReview => "approve one retry of a recent auto-review denial",
            SlashCommand::Memories => "configure memory use and generation",
            SlashCommand::Mcp => "list configured MCP tools; use /mcp verbose for details",
            SlashCommand::Apps => "manage apps",
            SlashCommand::Plugins => "browse plugins",
            SlashCommand::Logout => "log out of Codex",
            SlashCommand::Rollout => "print the rollout file path",
            SlashCommand::TestApproval => "test approval request",
        }
    }

    /// Command string without the leading '/'. Provided for compatibility with
    /// existing code that expects a method named `command()`.
    pub fn command(self) -> &'static str {
        self.into()
    }

    /// Short command spellings that should appear in slash autocomplete.
    ///
    /// These stay separate from `command()` so selecting an alias can preserve
    /// the short text in the composer while dispatch still resolves to the same
    /// underlying command.
    pub fn completion_aliases(self) -> &'static [&'static str] {
        match self {
            SlashCommand::Model => &["m"],
            SlashCommand::Effort => &["e"],
            SlashCommand::Reload => &["r"],
            SlashCommand::Compact => &["c"],
            SlashCommand::Id => &["i"],
            SlashCommand::Review => &["rev"],
            SlashCommand::Title => &["t"],
            SlashCommand::Retitle => &["rt"],
            SlashCommand::Emoji => &["em"],
            _ => &[],
        }
    }

    /// Whether this command supports inline args (for example `/review ...`).
    pub fn supports_inline_args(self) -> bool {
        matches!(
            self,
            SlashCommand::Review
                | SlashCommand::Rename
                | SlashCommand::Park
                | SlashCommand::Done
                | SlashCommand::Active
                | SlashCommand::Plan
                | SlashCommand::Goal
                | SlashCommand::Fast
                | SlashCommand::Ide
                | SlashCommand::Keymap
                | SlashCommand::Mcp
                | SlashCommand::Side
                | SlashCommand::Resume
                | SlashCommand::SandboxReadRoot
        )
    }

    /// Whether this command remains available inside an active side conversation.
    pub fn available_in_side_conversation(self) -> bool {
        matches!(
            self,
            SlashCommand::Id
                | SlashCommand::Copy
                | SlashCommand::CopyLastRequest
                | SlashCommand::Diff
                | SlashCommand::Mention
                | SlashCommand::Status
                | SlashCommand::Ide
        )
    }

    /// Whether this command can be run while a task is in progress.
    pub fn available_during_task(self) -> bool {
        match self {
            SlashCommand::New
            | SlashCommand::Resume
            | SlashCommand::Reload
            | SlashCommand::Fork
            | SlashCommand::Init
            | SlashCommand::Compact
            | SlashCommand::Model
            | SlashCommand::Effort
            | SlashCommand::Fast
            | SlashCommand::Personality
            | SlashCommand::Permissions
            | SlashCommand::Keymap
            | SlashCommand::Vim
            | SlashCommand::ElevateSandbox
            | SlashCommand::SandboxReadRoot
            | SlashCommand::Experimental
            | SlashCommand::Memories
            | SlashCommand::Review
            | SlashCommand::Retitle
            | SlashCommand::Emoji
            | SlashCommand::Plan
            | SlashCommand::Clear
            | SlashCommand::Logout
            | SlashCommand::MemoryDrop
            | SlashCommand::MemoryUpdate => false,
            SlashCommand::Diff
            | SlashCommand::Id
            | SlashCommand::Copy
            | SlashCommand::CopyLastRequest
            | SlashCommand::Rename
            | SlashCommand::Park
            | SlashCommand::Done
            | SlashCommand::Active
            | SlashCommand::Mention
            | SlashCommand::Skills
            | SlashCommand::Hooks
            | SlashCommand::Status
            | SlashCommand::DebugConfig
            | SlashCommand::Ps
            | SlashCommand::Stop
            | SlashCommand::Goal
            | SlashCommand::Mcp
            | SlashCommand::Apps
            | SlashCommand::Plugins
            | SlashCommand::Title
            | SlashCommand::Statusline
            | SlashCommand::AutoReview
            | SlashCommand::Feedback
            | SlashCommand::Ide
            | SlashCommand::Quit
            | SlashCommand::Exit
            | SlashCommand::Delete
            | SlashCommand::Side
            | SlashCommand::Condensed => true,
            SlashCommand::Rollout => true,
            SlashCommand::TestApproval => true,
            SlashCommand::Realtime => true,
            SlashCommand::Settings => true,
            SlashCommand::Collab => true,
            SlashCommand::Agent | SlashCommand::MultiAgents => true,
            SlashCommand::Theme => false,
        }
    }

    fn is_visible(self) -> bool {
        match self {
            SlashCommand::SandboxReadRoot => cfg!(target_os = "windows"),
            SlashCommand::Copy | SlashCommand::CopyLastRequest => !cfg!(target_os = "android"),
            SlashCommand::Rollout | SlashCommand::TestApproval => cfg!(debug_assertions),
            _ => true,
        }
    }
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    SlashCommand::iter()
        .filter(|command| command.is_visible())
        .map(|c| (c.command(), c))
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    use super::SlashCommand;

    #[test]
    fn stop_command_is_canonical_name() {
        assert_eq!(SlashCommand::Stop.command(), "stop");
    }

    #[test]
    fn clean_alias_parses_to_stop_command() {
        assert_eq!(SlashCommand::from_str("clean"), Ok(SlashCommand::Stop));
    }

    #[test]
    fn r_alias_parses_to_reload_command() {
        assert_eq!(SlashCommand::Reload.command(), "reload");
        assert_eq!(SlashCommand::from_str("r"), Ok(SlashCommand::Reload));
    }

    #[test]
    fn m_alias_parses_to_model_command() {
        assert_eq!(SlashCommand::Model.command(), "model");
        assert_eq!(SlashCommand::from_str("m"), Ok(SlashCommand::Model));
    }

    #[test]
    fn e_alias_parses_to_effort_command() {
        assert_eq!(SlashCommand::Effort.command(), "effort");
        assert_eq!(SlashCommand::from_str("e"), Ok(SlashCommand::Effort));
    }

    #[test]
    fn c_alias_parses_to_compact_command() {
        assert_eq!(SlashCommand::Compact.command(), "compact");
        assert_eq!(SlashCommand::from_str("c"), Ok(SlashCommand::Compact));
    }

    #[test]
    fn i_alias_parses_to_id_command() {
        assert_eq!(SlashCommand::Id.command(), "id");
        assert_eq!(SlashCommand::from_str("i"), Ok(SlashCommand::Id));
    }

    #[test]
    fn rev_alias_parses_to_review_command() {
        assert_eq!(SlashCommand::Review.command(), "review");
        assert_eq!(SlashCommand::from_str("rev"), Ok(SlashCommand::Review));
    }

    #[test]
    fn t_alias_parses_to_title_command() {
        assert_eq!(SlashCommand::Title.command(), "title");
        assert_eq!(SlashCommand::from_str("t"), Ok(SlashCommand::Title));
    }

    #[test]
    fn rt_alias_parses_to_retitle_command() {
        assert_eq!(SlashCommand::Retitle.command(), "retitle");
        assert_eq!(SlashCommand::from_str("rt"), Ok(SlashCommand::Retitle));
    }

    #[test]
    fn em_alias_parses_to_emoji_command() {
        assert_eq!(SlashCommand::Emoji.command(), "emoji");
        assert_eq!(SlashCommand::from_str("em"), Ok(SlashCommand::Emoji));
    }

    #[test]
    fn condensed_command_parses_to_condensed_command() {
        assert_eq!(SlashCommand::Condensed.command(), "condensed");
        assert_eq!(
            SlashCommand::from_str("condensed"),
            Ok(SlashCommand::Condensed)
        );
    }

    #[test]
    fn certain_commands_are_available_during_task() {
        assert!(SlashCommand::Goal.available_during_task());
        assert!(SlashCommand::Ide.available_during_task());
        assert!(SlashCommand::Title.available_during_task());
        assert!(SlashCommand::Statusline.available_during_task());
    }

    #[test]
    fn auto_review_command_is_approve() {
        assert_eq!(SlashCommand::AutoReview.command(), "approve");
        assert_eq!(
            SlashCommand::from_str("approve"),
            Ok(SlashCommand::AutoReview)
        );
    }
}
