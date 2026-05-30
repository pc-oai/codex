use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

/// The current Codex CLI version as embedded at compile time.
pub const CODEX_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Monotonic local generation for source builds.
///
/// `scripts/local-build-codex` bumps this before each runnable local build so
/// running source builds can be compared at a glance without relying on commits
/// or timestamps.
pub const LOCAL_BUILD_NUMBER: u32 = 113;

/// Monotonic local generation for source builds, if this is a source build.
pub fn local_build_number() -> Option<u32> {
    (CODEX_CLI_VERSION == "0.0.0").then_some(LOCAL_BUILD_NUMBER)
}

/// Compact footer label for source builds only.
pub fn local_build_label() -> Option<String> {
    local_build_number().map(|build_number| {
        let reload_marker = if local_reload_available(build_number) {
            " ↻"
        } else {
            ""
        };
        format!("v{build_number}{reload_marker}")
    })
}

fn local_reload_available(running_build: u32) -> bool {
    let Some(target_dir) = current_local_target_dir() else {
        return false;
    };
    let Some(codex_rs_dir) = target_dir.parent() else {
        return false;
    };
    let version_source = codex_rs_dir.join("tui/src/version.rs");
    let Some(latest_build) = latest_local_build_number(&version_source) else {
        return false;
    };
    if latest_build <= running_build {
        return false;
    }

    let Some(latest_binary_mtime) = newest_local_binary_mtime(&target_dir) else {
        return false;
    };
    let Ok(version_source_mtime) = fs::metadata(version_source).and_then(|meta| meta.modified())
    else {
        return false;
    };
    latest_binary_mtime >= version_source_mtime
}

fn current_local_target_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "target"))
        .map(Path::to_path_buf)
}

fn latest_local_build_number(version_source: &Path) -> Option<u32> {
    let source = fs::read_to_string(version_source).ok()?;
    source.lines().find_map(parse_local_build_number_line)
}

fn parse_local_build_number_line(line: &str) -> Option<u32> {
    line.strip_prefix("pub const LOCAL_BUILD_NUMBER: u32 = ")?
        .strip_suffix(';')?
        .parse()
        .ok()
}

fn newest_local_binary_mtime(target_dir: &Path) -> Option<SystemTime> {
    ["release/codex", "debug/codex"]
        .into_iter()
        .filter_map(|relative| {
            fs::metadata(target_dir.join(relative))
                .ok()
                .and_then(|meta| meta.modified().ok())
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::parse_local_build_number_line;

    #[test]
    fn parses_local_build_number_source_line() {
        assert_eq!(
            parse_local_build_number_line("pub const LOCAL_BUILD_NUMBER: u32 = 16;"),
            Some(16)
        );
        assert_eq!(
            parse_local_build_number_line("LOCAL_BUILD_NUMBER = 16"),
            None
        );
    }
}
