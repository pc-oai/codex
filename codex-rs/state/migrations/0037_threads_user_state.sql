ALTER TABLE threads ADD COLUMN user_state TEXT NOT NULL DEFAULT 'active';
CREATE INDEX idx_threads_user_state ON threads(user_state);
