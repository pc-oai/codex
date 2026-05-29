//! Recent agent-touched path menu support.

use std::path::Path;
use std::path::PathBuf;

use codex_app_server_protocol::FileUpdateChange;
use codex_app_server_protocol::PatchChangeKind;

use super::ChatWidget;
use crate::diff_render::display_path_for;
use crate::history_cell;

impl ChatWidget {
    pub(crate) fn open_touched_path_menu(&mut self) {
        let paths = self
            .transcript
            .recent_agent_touched_paths
            .iter()
            .rev()
            .map(|path| {
                (
                    display_path_for(path.as_path(), self.config.cwd.as_path()),
                    path.clone(),
                )
            })
            .collect::<Vec<_>>();
        if paths.is_empty() {
            self.add_to_history(history_cell::new_info_event(
                "No recently touched agent paths".into(),
                /*hint*/ None,
            ));
            self.request_redraw();
            return;
        }
        self.bottom_pane.show_touched_path_menu(paths);
        self.request_redraw();
    }

    pub(super) fn record_completed_file_change_paths(&mut self, changes: Vec<FileUpdateChange>) {
        let cwd = self.config.cwd.to_path_buf();
        let paths = changes
            .into_iter()
            .filter_map(|change| touched_path_for_change(change, cwd.as_path()));
        self.transcript.record_agent_touched_paths(paths);
    }
}

fn touched_path_for_change(change: FileUpdateChange, cwd: &Path) -> Option<PathBuf> {
    let path = match change.kind {
        PatchChangeKind::Delete => return None,
        PatchChangeKind::Add => PathBuf::from(change.path),
        PatchChangeKind::Update { move_path } => {
            move_path.unwrap_or_else(|| PathBuf::from(change.path))
        }
    };
    Some(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn update(path: &str, move_path: Option<PathBuf>) -> FileUpdateChange {
        FileUpdateChange {
            path: path.to_string(),
            kind: PatchChangeKind::Update { move_path },
            diff: String::new(),
        }
    }

    #[test]
    fn touched_path_for_change_uses_openable_paths() {
        let cwd = Path::new("/tmp/project");
        assert_eq!(
            touched_path_for_change(update("src/lib.rs", None), cwd),
            Some(cwd.join("src/lib.rs"))
        );
        assert_eq!(
            touched_path_for_change(update("src/old.rs", Some(PathBuf::from("src/new.rs"))), cwd),
            Some(cwd.join("src/new.rs"))
        );
        assert_eq!(
            touched_path_for_change(
                FileUpdateChange {
                    path: "gone.rs".to_string(),
                    kind: PatchChangeKind::Delete,
                    diff: String::new(),
                },
                cwd,
            ),
            None
        );
    }
}
