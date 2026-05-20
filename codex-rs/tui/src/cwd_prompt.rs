/// Action offered when the TUI asks how to continue from a cwd/session match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CwdPromptAction {
    Resume,
    Fork,
}
