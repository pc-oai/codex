use std::borrow::Cow;

use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;

pub(crate) static STATE_MIGRATOR: Migrator = sqlx::migrate!("./migrations");
pub(crate) static LOGS_MIGRATOR: Migrator = sqlx::migrate!("./logs_migrations");
pub(crate) static GOALS_MIGRATOR: Migrator = sqlx::migrate!("./goals_migrations");
pub(crate) static MEMORIES_MIGRATOR: Migrator = sqlx::migrate!("./memory_migrations");
pub(crate) static PC_STATE_MIGRATOR: Migrator = sqlx::migrate!("./pc_migrations");

const LEGACY_PC_METADATA_COPY_FLAG: &str = "legacy_threads_columns_copied";

/// Allow an older Codex binary to open a database that has already been
/// migrated by a newer binary running in parallel.
///
/// We intentionally ignore applied migration versions that are newer than the
/// embedded migration set. Known migration versions are still validated by
/// checksum, so this only relaxes the "database is ahead of me" case.
fn runtime_migrator(base: &'static Migrator) -> Migrator {
    Migrator {
        migrations: Cow::Borrowed(base.migrations.as_ref()),
        ignore_missing: true,
        locking: base.locking,
        no_tx: base.no_tx,
        table_name: base.table_name.clone(),
        create_schemas: base.create_schemas.clone(),
    }
}

pub(crate) fn runtime_state_migrator() -> Migrator {
    runtime_migrator(&STATE_MIGRATOR)
}

pub(crate) fn runtime_logs_migrator() -> Migrator {
    runtime_migrator(&LOGS_MIGRATOR)
}

pub(crate) fn runtime_goals_migrator() -> Migrator {
    runtime_migrator(&GOALS_MIGRATOR)
}

pub(crate) fn runtime_memories_migrator() -> Migrator {
    runtime_migrator(&MEMORIES_MIGRATOR)
}

/// Run fork-only state migrations in their own ledger from their first use.
///
/// Do not point `STATE_MIGRATOR` at this table. Its migration history belongs
/// to upstream and must remain recorded in `_sqlx_migrations`.
pub(crate) fn runtime_pc_state_migrator() -> Migrator {
    let mut migrator = runtime_migrator(&PC_STATE_MIGRATOR);
    migrator.dangerous_set_table_name("_pc_sqlx_migrations");
    migrator
}

/// Release earlier fork migration records from upstream's version range.
///
/// The legacy columns remain in place until their data is copied to
/// `pc_thread_metadata` after upstream migration validation completes.
pub(crate) async fn release_legacy_pc_state_migrations(pool: &SqlitePool) -> sqlx::Result<()> {
    let migration_table: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND name = '_sqlx_migrations'
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    if migration_table.is_none() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        "DELETE FROM _sqlx_migrations
         WHERE description IN (
             'threads user message count',
             'threads user message count known',
             'threads user state',
             'reset backfill for user message count',
             'keep backfill complete after user message count'
         )",
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await
}

/// Copy legacy fork columns to `pc_thread_metadata` and release upstream-owned
/// schema/ledger space.
///
/// This runs after `PC_STATE_MIGRATOR`, which creates the destination and flag
/// tables. The bridge is deliberately one-shot because later local writes only
/// update `pc_thread_metadata`.
pub(crate) async fn migrate_legacy_pc_thread_metadata(pool: &SqlitePool) -> sqlx::Result<()> {
    let copied: Option<String> =
        sqlx::query_scalar("SELECT name FROM pc_state_migration_flags WHERE name = ?")
            .bind(LEGACY_PC_METADATA_COPY_FLAG)
            .fetch_optional(pool)
            .await?;
    if copied.is_some() {
        return Ok(());
    }

    let rows = sqlx::query("PRAGMA table_info(threads)")
        .fetch_all(pool)
        .await?;
    let column_names = rows
        .into_iter()
        .map(|row| row.try_get::<String, _>("name"))
        .collect::<Result<std::collections::HashSet<_>, _>>()?;
    let legacy_columns = [
        "user_message_count",
        "user_message_count_known",
        "user_state",
    ];
    let legacy_column_count = legacy_columns
        .iter()
        .filter(|column| column_names.contains(**column))
        .count();
    if legacy_column_count != 0 && legacy_column_count != legacy_columns.len() {
        return Err(sqlx::Error::Protocol(
            "partial legacy pc thread metadata columns found in threads table".to_string(),
        ));
    }

    let mut tx = pool.begin().await?;
    if legacy_column_count == legacy_columns.len() {
        sqlx::query(
            "INSERT INTO pc_thread_metadata
             (thread_id, user_message_count, user_message_count_known, user_state)
             SELECT id, user_message_count, user_message_count_known, user_state
             FROM threads
             WHERE true
             ON CONFLICT(thread_id) DO UPDATE SET
                 user_message_count = excluded.user_message_count,
                 user_message_count_known = excluded.user_message_count_known,
                 user_state = excluded.user_state",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query("DROP INDEX IF EXISTS idx_threads_user_state")
            .execute(&mut *tx)
            .await?;
        for statement in [
            "ALTER TABLE threads DROP COLUMN user_message_count",
            "ALTER TABLE threads DROP COLUMN user_message_count_known",
            "ALTER TABLE threads DROP COLUMN user_state",
        ] {
            sqlx::query(statement).execute(&mut *tx).await?;
        }
    }

    sqlx::query(
        "DELETE FROM _sqlx_migrations
         WHERE description IN (
             'threads user message count',
             'threads user message count known',
             'threads user state',
             'reset backfill for user message count',
             'keep backfill complete after user message count'
         )",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO pc_state_migration_flags (name) VALUES (?)")
        .bind(LEGACY_PC_METADATA_COPY_FLAG)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use sqlx::sqlite::SqliteConnectOptions;
    use std::path::PathBuf;

    fn unique_temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "codex-state-migration-test-{}",
            uuid::Uuid::new_v4()
        ))
    }

    fn state_migrator_through(version: i64) -> Migrator {
        Migrator {
            migrations: Cow::Owned(
                STATE_MIGRATOR
                    .migrations
                    .iter()
                    .filter(|migration| migration.version <= version)
                    .cloned()
                    .collect(),
            ),
            ignore_missing: STATE_MIGRATOR.ignore_missing,
            locking: STATE_MIGRATOR.locking,
            no_tx: STATE_MIGRATOR.no_tx,
            table_name: STATE_MIGRATOR.table_name.clone(),
            create_schemas: STATE_MIGRATOR.create_schemas.clone(),
        }
    }

    async fn insert_applied_migration(pool: &SqlitePool, version: i64, description: &str) {
        let success = true;
        let execution_time = 1_i64;
        sqlx::query(
            "INSERT INTO _sqlx_migrations
             (version, description, success, checksum, execution_time)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(version)
        .bind(description)
        .bind(success)
        .bind(vec![version as u8])
        .bind(execution_time)
        .execute(pool)
        .await
        .expect("insert applied migration");
    }

    async fn add_legacy_columns(pool: &SqlitePool) {
        sqlx::query("ALTER TABLE threads ADD COLUMN user_message_count INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await
            .expect("add legacy user message count");
        sqlx::query(
            "ALTER TABLE threads ADD COLUMN user_message_count_known INTEGER NOT NULL DEFAULT 0",
        )
        .execute(pool)
        .await
        .expect("add legacy known user message count flag");
        sqlx::query("ALTER TABLE threads ADD COLUMN user_state TEXT NOT NULL DEFAULT 'active'")
            .execute(pool)
            .await
            .expect("add legacy user state");
        sqlx::query("CREATE INDEX idx_threads_user_state ON threads(user_state)")
            .execute(pool)
            .await
            .expect("index legacy user state");
    }

    async fn insert_thread_with_legacy_values(pool: &SqlitePool) {
        sqlx::query(
            "INSERT INTO threads
             (id, rollout_path, created_at, updated_at, source, model_provider, cwd, title,
              sandbox_policy, approval_mode, user_message_count, user_message_count_known,
              user_state)
             VALUES ('thread-1', '/tmp/thread.jsonl', 1, 1, 'cli', 'openai', '/tmp', '',
                     'read-only', 'never', 7, 1, 'parked')",
        )
        .execute(pool)
        .await
        .expect("insert legacy thread");
    }

    async fn run_pc_migrations_and_bridge(pool: &SqlitePool) {
        runtime_pc_state_migrator()
            .run(pool)
            .await
            .expect("apply pc state migrations");
        migrate_legacy_pc_thread_metadata(pool)
            .await
            .expect("copy legacy pc thread metadata");
    }

    #[tokio::test]
    async fn migrates_applied_local_columns_from_repaired_database() {
        let codex_home = unique_temp_dir();
        tokio::fs::create_dir_all(&codex_home)
            .await
            .expect("create codex home");
        let state_path = crate::state_db_path(codex_home.as_path());
        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .filename(&state_path)
                .create_if_missing(true),
        )
        .await
        .expect("open state db");
        runtime_state_migrator()
            .run(&pool)
            .await
            .expect("apply upstream state migrations");
        add_legacy_columns(&pool).await;
        insert_thread_with_legacy_values(&pool).await;
        insert_applied_migration(&pool, 36, "threads user message count").await;
        insert_applied_migration(&pool, 37, "threads user message count known").await;
        insert_applied_migration(&pool, 38, "threads user state").await;

        release_legacy_pc_state_migrations(&pool)
            .await
            .expect("release legacy fork migrations");
        run_pc_migrations_and_bridge(&pool).await;
        let pc_metadata = sqlx::query_as::<_, (i64, bool, String)>(
            "SELECT user_message_count, user_message_count_known, user_state
             FROM pc_thread_metadata WHERE thread_id = 'thread-1'",
        )
        .fetch_one(&pool)
        .await
        .expect("read pc metadata");
        let local_migrations = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM _sqlx_migrations
             WHERE description LIKE 'threads user %'",
        )
        .fetch_one(&pool)
        .await
        .expect("count local migrations");
        let legacy_columns = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pragma_table_info('threads')
             WHERE name IN ('user_message_count', 'user_message_count_known', 'user_state')",
        )
        .fetch_one(&pool)
        .await
        .expect("count legacy columns");

        assert_eq!(pc_metadata, (7, true, "parked".to_string()));
        assert_eq!(local_migrations, 0);
        assert_eq!(legacy_columns, 0);

        pool.close().await;
        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn releases_conflicting_local_migration_35_before_upstream_migration_35() {
        let codex_home = unique_temp_dir();
        tokio::fs::create_dir_all(&codex_home)
            .await
            .expect("create codex home");
        let state_path = crate::state_db_path(codex_home.as_path());
        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .filename(&state_path)
                .create_if_missing(true),
        )
        .await
        .expect("open state db");
        state_migrator_through(34)
            .run(&pool)
            .await
            .expect("apply state schema before official migration 35");
        add_legacy_columns(&pool).await;
        insert_thread_with_legacy_values(&pool).await;
        insert_applied_migration(&pool, 35, "threads user message count").await;
        insert_applied_migration(&pool, 36, "threads user message count known").await;
        insert_applied_migration(&pool, 37, "threads user state").await;

        release_legacy_pc_state_migrations(&pool)
            .await
            .expect("release conflicting local migrations");
        runtime_state_migrator()
            .run(&pool)
            .await
            .expect("apply official migration 35");
        run_pc_migrations_and_bridge(&pool).await;
        let migrations = sqlx::query_as::<_, (i64, String)>(
            "SELECT version, description FROM _sqlx_migrations WHERE version >= 35 ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .expect("read upstream migrations");

        assert_eq!(migrations, vec![(35, "drop memory tables".to_string())]);

        pool.close().await;
        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
