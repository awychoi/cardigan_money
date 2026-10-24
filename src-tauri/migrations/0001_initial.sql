-- Migration 1: audit metadata, settings, and the security event trail.
-- KDF parameters and the wrapped DEK live only in vault.header, never here.

CREATE TABLE vault_metadata (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    vault_id TEXT NOT NULL CHECK (length(vault_id) = 32),
    cipher_version TEXT NOT NULL CHECK (length(cipher_version) BETWEEN 1 AND 64),
    cipher_provider TEXT NOT NULL CHECK (length(cipher_provider) BETWEEN 1 AND 64),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    generation INTEGER NOT NULL CHECK (generation >= 1),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    rotated_at TEXT CHECK (rotated_at IS NULL OR length(rotated_at) = 20),
    last_integrity_check_at TEXT CHECK (last_integrity_check_at IS NULL OR length(last_integrity_check_at) = 20)
) STRICT;

CREATE TABLE settings (
    key TEXT PRIMARY KEY CHECK (length(key) BETWEEN 1 AND 64),
    value TEXT NOT NULL CHECK (length(value) <= 8192),
    updated_at TEXT NOT NULL CHECK (length(updated_at) = 20)
) STRICT;

CREATE TABLE security_events (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    event_type TEXT NOT NULL CHECK (event_type IN (
        'vault_created', 'unlock_success', 'unlock_failure', 'lock',
        'backup_created', 'backup_verified', 'backup_failed',
        'restore_started', 'restore_succeeded', 'restore_failed',
        'export', 'rekey', 'credential_added', 'credential_removed',
        'integrity_failure', 'single_instance_rejected',
        'migration_applied', 'migration_failed', 'demo_reset', 'delete_all',
        'sync_started', 'sync_finished', 'settings_changed'
    )),
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'info')),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    metadata_json TEXT CHECK (metadata_json IS NULL OR length(metadata_json) <= 1024)
) STRICT;

CREATE INDEX security_events_created_idx ON security_events (created_at DESC);
CREATE INDEX security_events_type_idx ON security_events (event_type, created_at DESC);
