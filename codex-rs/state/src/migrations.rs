use std::borrow::Cow;

use sqlx::SqlitePool;
use sqlx::migrate::Migrator;

pub(crate) static STATE_MIGRATOR: Migrator = sqlx::migrate!("./migrations");
pub(crate) static LOGS_MIGRATOR: Migrator = sqlx::migrate!("./logs_migrations");
pub(crate) static GOALS_MIGRATOR: Migrator = sqlx::migrate!("./goals_migrations");
pub(crate) static MEMORIES_MIGRATOR: Migrator = sqlx::migrate!("./memory_migrations");

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

/// Renumber local-fork state migrations that were merged after upstream reused
/// the same version range for different state migrations.
pub(crate) async fn remap_legacy_state_migrations(pool: &SqlitePool) -> sqlx::Result<()> {
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
    for (legacy_version, current_version, description) in [
        (37_i64, 38_i64, "threads user state"),
        (36_i64, 37_i64, "threads user message count known"),
        (35_i64, 36_i64, "threads user message count"),
        (34_i64, 38_i64, "threads user state"),
        (33_i64, 37_i64, "threads user message count known"),
        (30_i64, 36_i64, "threads user message count"),
    ] {
        sqlx::query(
            "UPDATE _sqlx_migrations
             SET version = ?
             WHERE version = ? AND description = ?",
        )
        .bind(current_version)
        .bind(legacy_version)
        .bind(description)
        .execute(&mut *tx)
        .await?;
    }

    for (legacy_version, description) in [
        (31_i64, "reset backfill for user message count"),
        (32_i64, "keep backfill complete after user message count"),
    ] {
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ? AND description = ?")
            .bind(legacy_version)
            .bind(description)
            .execute(&mut *tx)
            .await?;
    }

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

    fn state_migrator_before_local_conflicts() -> Migrator {
        Migrator {
            migrations: Cow::Owned(
                STATE_MIGRATOR
                    .migrations
                    .iter()
                    .filter(|migration| migration.version <= 29)
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

    fn state_migration_checksum(version: i64) -> Vec<u8> {
        STATE_MIGRATOR
            .migrations
            .iter()
            .find(|migration| migration.version == version)
            .unwrap_or_else(|| panic!("missing test migration {version}"))
            .checksum
            .to_vec()
    }

    async fn insert_applied_migration(
        pool: &SqlitePool,
        version: i64,
        description: &str,
        checksum: Vec<u8>,
    ) {
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
        .bind(checksum)
        .bind(execution_time)
        .execute(pool)
        .await
        .expect("insert applied migration");
    }

    #[tokio::test]
    async fn remaps_conflicting_local_migrations_before_current_state_migrations() {
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
        state_migrator_before_local_conflicts()
            .run(&pool)
            .await
            .expect("apply state schema before conflicting local migrations");
        sqlx::query("ALTER TABLE threads ADD COLUMN user_message_count INTEGER NOT NULL DEFAULT 0")
            .execute(&pool)
            .await
            .expect("add legacy user message count");
        insert_applied_migration(
            &pool,
            30,
            "threads user message count",
            state_migration_checksum(36),
        )
        .await;
        insert_applied_migration(&pool, 31, "reset backfill for user message count", vec![31])
            .await;
        insert_applied_migration(
            &pool,
            32,
            "keep backfill complete after user message count",
            vec![32],
        )
        .await;
        sqlx::query(
            "ALTER TABLE threads ADD COLUMN user_message_count_known INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&pool)
        .await
        .expect("add legacy known user message count flag");
        insert_applied_migration(
            &pool,
            33,
            "threads user message count known",
            state_migration_checksum(37),
        )
        .await;
        sqlx::query("ALTER TABLE threads ADD COLUMN user_state TEXT NOT NULL DEFAULT 'active'")
            .execute(&pool)
            .await
            .expect("add legacy user state");
        sqlx::query("CREATE INDEX idx_threads_user_state ON threads(user_state)")
            .execute(&pool)
            .await
            .expect("index legacy user state");
        insert_applied_migration(
            &pool,
            34,
            "threads user state",
            state_migration_checksum(38),
        )
        .await;

        remap_legacy_state_migrations(&pool)
            .await
            .expect("remap legacy migrations");
        runtime_state_migrator()
            .run(&pool)
            .await
            .expect("apply current state migrations");
        let remapped_migrations = sqlx::query_as::<_, (i64, String)>(
            "SELECT version, description
             FROM _sqlx_migrations
             WHERE version >= 30
             ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .expect("read remapped migrations");

        assert_eq!(
            remapped_migrations,
            vec![
                (30, "threads thread source".to_string()),
                (31, "drop device key bindings".to_string()),
                (32, "threads preview".to_string()),
                (33, "thread goal stopped statuses".to_string()),
                (34, "drop thread goals".to_string()),
                (35, "drop memory tables".to_string()),
                (36, "threads user message count".to_string()),
                (37, "threads user message count known".to_string()),
                (38, "threads user state".to_string()),
            ]
        );

        pool.close().await;
        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn remaps_applied_local_migration_35_before_upstream_migration_35() {
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
        Migrator {
            migrations: Cow::Owned(
                STATE_MIGRATOR
                    .migrations
                    .iter()
                    .filter(|migration| migration.version <= 34)
                    .cloned()
                    .collect(),
            ),
            ignore_missing: STATE_MIGRATOR.ignore_missing,
            locking: STATE_MIGRATOR.locking,
            no_tx: STATE_MIGRATOR.no_tx,
            table_name: STATE_MIGRATOR.table_name.clone(),
            create_schemas: STATE_MIGRATOR.create_schemas.clone(),
        }
        .run(&pool)
        .await
        .expect("apply upstream state schema before migration 35");
        sqlx::query("ALTER TABLE threads ADD COLUMN user_message_count INTEGER NOT NULL DEFAULT 0")
            .execute(&pool)
            .await
            .expect("add local user message count");
        sqlx::query(
            "ALTER TABLE threads ADD COLUMN user_message_count_known INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&pool)
        .await
        .expect("add local known user message count flag");
        sqlx::query("ALTER TABLE threads ADD COLUMN user_state TEXT NOT NULL DEFAULT 'active'")
            .execute(&pool)
            .await
            .expect("add local user state");
        sqlx::query("CREATE INDEX idx_threads_user_state ON threads(user_state)")
            .execute(&pool)
            .await
            .expect("index local user state");
        insert_applied_migration(
            &pool,
            35,
            "threads user message count",
            state_migration_checksum(36),
        )
        .await;
        insert_applied_migration(
            &pool,
            36,
            "threads user message count known",
            state_migration_checksum(37),
        )
        .await;
        insert_applied_migration(
            &pool,
            37,
            "threads user state",
            state_migration_checksum(38),
        )
        .await;

        remap_legacy_state_migrations(&pool)
            .await
            .expect("remap applied local migrations");
        runtime_state_migrator()
            .run(&pool)
            .await
            .expect("apply upstream migration 35");
        let migrations = sqlx::query_as::<_, (i64, String)>(
            "SELECT version, description FROM _sqlx_migrations WHERE version >= 35 ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .expect("read remapped migrations");

        assert_eq!(
            migrations,
            vec![
                (35, "drop memory tables".to_string()),
                (36, "threads user message count".to_string()),
                (37, "threads user message count known".to_string()),
                (38, "threads user state".to_string()),
            ]
        );

        pool.close().await;
        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
