CREATE TABLE pc_thread_metadata (
    thread_id TEXT PRIMARY KEY NOT NULL,
    user_message_count INTEGER NOT NULL DEFAULT 0,
    user_message_count_known INTEGER NOT NULL DEFAULT 0,
    user_state TEXT NOT NULL DEFAULT 'active',
    FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
);

CREATE INDEX idx_pc_thread_metadata_user_state ON pc_thread_metadata(user_state);

CREATE TABLE pc_state_migration_flags (
    name TEXT PRIMARY KEY NOT NULL
);
