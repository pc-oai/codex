/// The current Codex CLI version as embedded at compile time.
pub const CODEX_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Monotonic local generation for source builds.
///
/// Bump this once for each local Codex change set so running source builds can
/// be compared at a glance without relying on commits or timestamps.
pub const LOCAL_BUILD_NUMBER: u32 = 2;

/// Compact footer label for source builds only.
pub fn local_build_label() -> Option<String> {
    (CODEX_CLI_VERSION == "0.0.0").then(|| format!("v{LOCAL_BUILD_NUMBER}"))
}
