use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;

use super::LocalThreadStore;
use super::helpers::scoped_rollout_path;
use crate::DeleteThreadParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn delete_thread(
    store: &LocalThreadStore,
    params: DeleteThreadParams,
) -> ThreadStoreResult<()> {
    let thread_id = params.thread_id;
    let state_db_ctx = store.state_db().await;
    let active_path = find_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        &thread_id.to_string(),
        state_db_ctx.as_deref(),
    )
    .await
    .map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("failed to locate thread id {thread_id}: {err}"),
    })?;
    let archived_path = if active_path.is_none() {
        find_archived_thread_path_by_id_str(
            store.config.codex_home.as_path(),
            &thread_id.to_string(),
            state_db_ctx.as_deref(),
        )
        .await
        .map_err(|err| ThreadStoreError::InvalidRequest {
            message: format!("failed to locate archived thread id {thread_id}: {err}"),
        })?
    } else {
        None
    };

    if let Some(path) = active_path {
        let canonical_path = scoped_rollout_path(
            store.config.codex_home.join(codex_rollout::SESSIONS_SUBDIR),
            path.as_path(),
            "sessions",
        )?;
        std::fs::remove_file(canonical_path).map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to delete thread: {err}"),
        })?;
    } else if let Some(path) = archived_path {
        let canonical_path = scoped_rollout_path(
            store
                .config
                .codex_home
                .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
            path.as_path(),
            "archived",
        )?;
        std::fs::remove_file(canonical_path).map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to delete thread: {err}"),
        })?;
    }

    if let Some(ctx) = state_db_ctx {
        let _ = ctx.delete_thread(thread_id).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_archived_session_file;
    use crate::local::test_support::write_session_file;

    #[tokio::test]
    async fn delete_thread_removes_active_rollout() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(301);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");

        store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect("delete thread");

        assert!(!active_path.exists());
    }

    #[tokio::test]
    async fn delete_thread_removes_archived_rollout_and_sqlite_metadata() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(302);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T12-00-00", uuid)
            .expect("archived session file");
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            archived_path.clone(),
            Utc::now(),
            SessionSource::Cli,
        );
        builder.model_provider = Some(config.default_model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let mut metadata = builder.build(config.default_model_provider_id.as_str());
        metadata.archived_at = Some(metadata.updated_at);
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect("delete thread");

        assert!(!archived_path.exists());
        assert_eq!(
            runtime
                .get_thread(thread_id)
                .await
                .expect("state db read should succeed"),
            None
        );
    }
}
