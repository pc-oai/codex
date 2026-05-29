//! Composer draft state preserved across TUI reloads and later resumes.
//!
//! Reload is intentionally a fresh process so the newest local binary takes over.
//! That process boundary used to discard the visible composer draft. Keep just
//! enough state on disk for the resumed process to restore the draft, then delete
//! the handoff immediately after reading it.
//!
//! Ordinary user exits use the same draft payload, but a separate directory. That
//! keeps a live reload handoff higher priority than an older quit/resume draft,
//! while still letting an unsent composer survive a later `resume`.

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::user_input::TextElement;
use serde::Deserialize;
use serde::Serialize;

const RELOAD_HANDOFF_DIR: &str = "reload-handoff";
const RELOAD_TREE_HANDOFF_DIR: &str = "reload-tree-handoff";
const RESUME_DRAFT_DIR: &str = "resume-drafts";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReloadMentionBinding {
    #[serde(default = "default_reload_mention_sigil")]
    pub(crate) sigil: char,
    pub(crate) mention: String,
    pub(crate) path: String,
}

fn default_reload_mention_sigil() -> char {
    '$'
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ReloadDraft {
    pub(crate) text: String,
    pub(crate) text_elements: Vec<TextElement>,
    pub(crate) local_image_paths: Vec<PathBuf>,
    pub(crate) remote_image_urls: Vec<String>,
    pub(crate) mention_bindings: Vec<ReloadMentionBinding>,
    pub(crate) pending_pastes: Vec<(String, String)>,
    pub(crate) cursor: usize,
}

impl ReloadDraft {
    pub(crate) fn has_content(&self) -> bool {
        !self.text.is_empty()
            || !self.text_elements.is_empty()
            || !self.local_image_paths.is_empty()
            || !self.remote_image_urls.is_empty()
            || !self.mention_bindings.is_empty()
            || !self.pending_pastes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ReloadDraftEnvelope {
    thread_id: ThreadId,
    draft: ReloadDraft,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ReloadTreeHandoff {
    pub(crate) root_thread_id: ThreadId,
    pub(crate) selected_thread_id: ThreadId,
    pub(crate) selected_draft: Option<ReloadDraft>,
}

#[cfg(test)]
pub(crate) fn save(codex_home: &Path, thread_id: ThreadId, draft: &ReloadDraft) -> io::Result<()> {
    save_in_dir(codex_home, RELOAD_HANDOFF_DIR, thread_id, draft)
}

pub(crate) fn take(codex_home: &Path, thread_id: ThreadId) -> io::Result<Option<ReloadDraft>> {
    take_in_dir(codex_home, RELOAD_HANDOFF_DIR, thread_id)
}

/// Persist enough context for a fresh process to reopen the same agent tree and visible thread.
pub(crate) fn save_tree(
    codex_home: &Path,
    root_thread_id: ThreadId,
    selected_thread_id: ThreadId,
    selected_draft: Option<&ReloadDraft>,
) -> io::Result<()> {
    let dir = draft_dir(codex_home, RELOAD_TREE_HANDOFF_DIR);
    fs::create_dir_all(&dir)?;
    let path = draft_path(codex_home, RELOAD_TREE_HANDOFF_DIR, root_thread_id);
    let temp_path = dir.join(format!("{root_thread_id}.{}.tmp", std::process::id()));
    let envelope = ReloadTreeHandoff {
        root_thread_id,
        selected_thread_id,
        selected_draft: selected_draft.cloned().filter(ReloadDraft::has_content),
    };
    let payload = serde_json::to_vec(&envelope).map_err(io::Error::other)?;
    fs::write(&temp_path, payload)?;
    fs::rename(temp_path, path)?;
    Ok(())
}

/// Consume the one-shot tree reload context for the root thread being resumed.
pub(crate) fn take_tree(
    codex_home: &Path,
    root_thread_id: ThreadId,
) -> io::Result<Option<ReloadTreeHandoff>> {
    let path = draft_path(codex_home, RELOAD_TREE_HANDOFF_DIR, root_thread_id);
    let payload = match fs::read(&path) {
        Ok(payload) => payload,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let envelope: ReloadTreeHandoff = serde_json::from_slice(&payload).map_err(io::Error::other)?;
    fs::remove_file(&path)?;
    if envelope.root_thread_id != root_thread_id {
        return Ok(None);
    }
    clear_resume(codex_home, envelope.selected_thread_id)?;
    if envelope.root_thread_id != envelope.selected_thread_id {
        clear_resume(codex_home, envelope.root_thread_id)?;
    }
    Ok(Some(envelope))
}

/// Replace the durable quit/resume draft for one thread.
///
/// Passing `None` clears any older durable draft so a user who resumes,
/// deletes the draft, and quits again does not see stale text later.
pub(crate) fn replace_resume(
    codex_home: &Path,
    thread_id: ThreadId,
    draft: Option<&ReloadDraft>,
) -> io::Result<()> {
    match draft {
        Some(draft) if draft.has_content() => {
            save_in_dir(codex_home, RESUME_DRAFT_DIR, thread_id, draft)
        }
        _ => clear_resume(codex_home, thread_id),
    }
}

/// Restore whichever draft should win for a resumed thread.
///
/// A live reload handoff is fresher than a prior quit/resume draft. When one is
/// present, clear the older durable draft so it cannot reappear on a later resume.
pub(crate) fn take_for_resume(
    codex_home: &Path,
    thread_id: ThreadId,
) -> io::Result<Option<ReloadDraft>> {
    if let Some(draft) = take(codex_home, thread_id)? {
        clear_resume(codex_home, thread_id)?;
        return Ok(Some(draft));
    }
    take_in_dir(codex_home, RESUME_DRAFT_DIR, thread_id)
}

pub(crate) fn clear_resume(codex_home: &Path, thread_id: ThreadId) -> io::Result<()> {
    let path = draft_path(codex_home, RESUME_DRAFT_DIR, thread_id);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn save_in_dir(
    codex_home: &Path,
    dir_name: &str,
    thread_id: ThreadId,
    draft: &ReloadDraft,
) -> io::Result<()> {
    if !draft.has_content() {
        return Ok(());
    }

    let dir = draft_dir(codex_home, dir_name);
    fs::create_dir_all(&dir)?;
    let path = draft_path(codex_home, dir_name, thread_id);
    let temp_path = dir.join(format!("{thread_id}.{}.tmp", std::process::id()));
    let envelope = ReloadDraftEnvelope {
        thread_id,
        draft: draft.clone(),
    };
    let payload = serde_json::to_vec(&envelope).map_err(io::Error::other)?;
    fs::write(&temp_path, payload)?;
    fs::rename(temp_path, path)?;
    Ok(())
}

fn take_in_dir(
    codex_home: &Path,
    dir_name: &str,
    thread_id: ThreadId,
) -> io::Result<Option<ReloadDraft>> {
    let path = draft_path(codex_home, dir_name, thread_id);
    let payload = match fs::read(&path) {
        Ok(payload) => payload,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let envelope: ReloadDraftEnvelope =
        serde_json::from_slice(&payload).map_err(io::Error::other)?;
    fs::remove_file(&path)?;
    if envelope.thread_id != thread_id {
        return Ok(None);
    }
    Ok(Some(envelope.draft))
}

fn draft_dir(codex_home: &Path, dir_name: &str) -> PathBuf {
    codex_home.join(dir_name)
}

fn draft_path(codex_home: &Path, dir_name: &str, thread_id: ThreadId) -> PathBuf {
    draft_dir(codex_home, dir_name).join(format!("{thread_id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_draft() -> ReloadDraft {
        ReloadDraft {
            text: "draft".to_string(),
            text_elements: Vec::new(),
            local_image_paths: vec![PathBuf::from("/tmp/image.png")],
            remote_image_urls: vec!["https://example.com/image.png".to_string()],
            mention_bindings: vec![ReloadMentionBinding {
                sigil: '$',
                mention: "docs".to_string(),
                path: "app://docs".to_string(),
            }],
            pending_pastes: vec![("id".to_string(), "payload".to_string())],
            cursor: 2,
        }
    }

    #[test]
    fn handoff_round_trips_once() {
        let temp = tempdir().expect("tempdir");
        let thread_id = ThreadId::new();
        let draft = sample_draft();

        save(temp.path(), thread_id, &draft).expect("save handoff");
        assert_eq!(
            take(temp.path(), thread_id).expect("take handoff"),
            Some(draft)
        );
        assert_eq!(take(temp.path(), thread_id).expect("take again"), None);
    }

    #[test]
    fn empty_draft_does_not_create_handoff() {
        let temp = tempdir().expect("tempdir");
        let thread_id = ThreadId::new();
        let draft = ReloadDraft {
            text: String::new(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
            mention_bindings: Vec::new(),
            pending_pastes: Vec::new(),
            cursor: 0,
        };

        save(temp.path(), thread_id, &draft).expect("save empty handoff");
        assert_eq!(take(temp.path(), thread_id).expect("take empty"), None);
    }

    #[test]
    fn resume_draft_round_trips_once() {
        let temp = tempdir().expect("tempdir");
        let thread_id = ThreadId::new();
        let draft = sample_draft();

        replace_resume(temp.path(), thread_id, Some(&draft)).expect("save resume draft");
        assert_eq!(
            take_for_resume(temp.path(), thread_id).expect("take resume draft"),
            Some(draft)
        );
        assert_eq!(
            take_for_resume(temp.path(), thread_id).expect("take resume draft again"),
            None
        );
    }

    #[test]
    fn empty_resume_replacement_clears_stale_draft() {
        let temp = tempdir().expect("tempdir");
        let thread_id = ThreadId::new();
        let draft = sample_draft();

        replace_resume(temp.path(), thread_id, Some(&draft)).expect("save resume draft");
        replace_resume(temp.path(), thread_id, /*draft*/ None).expect("clear resume draft");

        assert_eq!(
            take_for_resume(temp.path(), thread_id).expect("take cleared resume draft"),
            None
        );
    }

    #[test]
    fn reload_handoff_wins_over_older_resume_draft() {
        let temp = tempdir().expect("tempdir");
        let thread_id = ThreadId::new();
        let resume_draft = sample_draft();
        let mut reload_draft = sample_draft();
        reload_draft.text = "newer reload draft".to_string();

        replace_resume(temp.path(), thread_id, Some(&resume_draft)).expect("save resume draft");
        save(temp.path(), thread_id, &reload_draft).expect("save reload handoff");

        assert_eq!(
            take_for_resume(temp.path(), thread_id).expect("take preferred draft"),
            Some(reload_draft)
        );
        assert_eq!(
            take_for_resume(temp.path(), thread_id).expect("stale resume cleared"),
            None
        );
    }

    #[test]
    fn tree_handoff_round_trips_root_selection_and_draft_once() {
        let temp = tempdir().expect("tempdir");
        let root_thread_id = ThreadId::new();
        let selected_thread_id = ThreadId::new();
        let draft = sample_draft();

        save_tree(
            temp.path(),
            root_thread_id,
            selected_thread_id,
            Some(&draft),
        )
        .expect("save tree handoff");

        assert_eq!(
            take_tree(temp.path(), root_thread_id).expect("take tree handoff"),
            Some(ReloadTreeHandoff {
                root_thread_id,
                selected_thread_id,
                selected_draft: Some(draft),
            })
        );
        assert_eq!(
            take_tree(temp.path(), root_thread_id).expect("take tree handoff again"),
            None
        );
    }

    #[test]
    fn tree_handoff_keeps_selection_even_without_a_draft() {
        let temp = tempdir().expect("tempdir");
        let root_thread_id = ThreadId::new();
        let selected_thread_id = ThreadId::new();

        save_tree(
            temp.path(),
            root_thread_id,
            selected_thread_id,
            /*selected_draft*/ None,
        )
        .expect("save tree handoff");

        assert_eq!(
            take_tree(temp.path(), root_thread_id).expect("take tree handoff"),
            Some(ReloadTreeHandoff {
                root_thread_id,
                selected_thread_id,
                selected_draft: None,
            })
        );
    }

    #[test]
    fn tree_handoff_clears_stale_resume_drafts_for_root_and_selection() {
        let temp = tempdir().expect("tempdir");
        let root_thread_id = ThreadId::new();
        let selected_thread_id = ThreadId::new();
        let draft = sample_draft();

        replace_resume(temp.path(), root_thread_id, Some(&draft)).expect("save root resume draft");
        replace_resume(temp.path(), selected_thread_id, Some(&draft))
            .expect("save selection resume draft");
        save_tree(
            temp.path(),
            root_thread_id,
            selected_thread_id,
            Some(&draft),
        )
        .expect("save tree handoff");

        assert!(
            take_tree(temp.path(), root_thread_id)
                .expect("take tree handoff")
                .is_some()
        );
        assert_eq!(
            take_for_resume(temp.path(), root_thread_id).expect("root draft cleared"),
            None
        );
        assert_eq!(
            take_for_resume(temp.path(), selected_thread_id).expect("selection draft cleared"),
            None
        );
    }
}
