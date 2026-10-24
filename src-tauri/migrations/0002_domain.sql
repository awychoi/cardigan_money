-- Migration 2: financial core. Money is signed integer minor units. Dates that
-- represent financial calendar days are date-only TEXT (YYYY-MM-DD). Provider
-- and CSV text is bounded and untrusted. No secrets or raw payloads are stored.

CREATE TABLE accounts (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    issuer TEXT NOT NULL CHECK (length(issuer) BETWEEN 1 AND 80),
    display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 120),
    last_four TEXT CHECK (last_four IS NULL OR (length(last_four) = 4 AND last_four NOT GLOB '*[^0-9]*')),
    account_type TEXT NOT NULL CHECK (account_type IN ('credit_card', 'charge_card', 'line_of_credit', 'other')),
    currency TEXT NOT NULL CHECK (length(currency) = 3 AND currency = upper(currency)),
    credit_limit_minor INTEGER CHECK (credit_limit_minor IS NULL OR credit_limit_minor > 0),
    include_in_overview INTEGER NOT NULL DEFAULT 1 CHECK (include_in_overview IN (0, 1)),
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('manual', 'provider', 'demo')),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    updated_at TEXT NOT NULL CHECK (length(updated_at) = 20)
) STRICT;

CREATE INDEX accounts_active_idx ON accounts (archived, include_in_overview);

CREATE TABLE connections (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    provider TEXT NOT NULL CHECK (provider IN ('plaid', 'mock')),
    environment TEXT NOT NULL CHECK (environment IN ('sandbox', 'production', 'demo')),
    institution_id TEXT CHECK (institution_id IS NULL OR length(institution_id) BETWEEN 1 AND 64),
    institution_name TEXT CHECK (institution_name IS NULL OR length(institution_name) BETWEEN 1 AND 120),
    credential_ref TEXT NOT NULL CHECK (length(credential_ref) = 36),
    status TEXT NOT NULL CHECK (status IN ('healthy', 'reconnect_required', 'error', 'disconnected')),
    last_attempt_at TEXT CHECK (last_attempt_at IS NULL OR length(last_attempt_at) = 20),
    last_success_at TEXT CHECK (last_success_at IS NULL OR length(last_success_at) = 20),
    cursor TEXT CHECK (cursor IS NULL OR length(cursor) <= 4096),
    sanitized_error TEXT CHECK (sanitized_error IS NULL OR length(sanitized_error) <= 256),
    last_remote_revocation_at TEXT CHECK (last_remote_revocation_at IS NULL OR length(last_remote_revocation_at) = 20),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    updated_at TEXT NOT NULL CHECK (length(updated_at) = 20)
) STRICT;

CREATE INDEX connections_status_idx ON connections (status);

CREATE TABLE account_sources (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    connection_id TEXT NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    provider_account_id TEXT NOT NULL CHECK (length(provider_account_id) BETWEEN 1 AND 200),
    provider_mask TEXT CHECK (provider_mask IS NULL OR length(provider_mask) <= 16),
    linked_at TEXT NOT NULL CHECK (length(linked_at) = 20),
    unlinked_at TEXT CHECK (unlinked_at IS NULL OR length(unlinked_at) = 20),
    UNIQUE (connection_id, provider_account_id)
) STRICT;

CREATE TABLE balance_snapshots (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    current_balance_minor INTEGER NOT NULL CHECK (current_balance_minor BETWEEN -1000000000000 AND 1000000000000),
    available_credit_minor INTEGER CHECK (available_credit_minor IS NULL OR available_credit_minor BETWEEN 0 AND 1000000000000),
    credit_limit_minor INTEGER CHECK (credit_limit_minor IS NULL OR credit_limit_minor BETWEEN 0 AND 1000000000000),
    currency TEXT NOT NULL CHECK (length(currency) = 3 AND currency = upper(currency)),
    retrieved_at TEXT NOT NULL CHECK (length(retrieved_at) = 20),
    provider_as_of TEXT CHECK (provider_as_of IS NULL OR length(provider_as_of) = 20),
    provenance TEXT NOT NULL CHECK (provenance IN ('provider', 'manual', 'demo')),
    source_connection_id TEXT REFERENCES connections(id) ON DELETE SET NULL
) STRICT;

CREATE INDEX balance_snapshots_account_idx ON balance_snapshots (account_id, retrieved_at DESC);

CREATE TABLE payment_snapshots (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    next_due_date TEXT CHECK (next_due_date IS NULL OR next_due_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    minimum_due_minor INTEGER CHECK (minimum_due_minor IS NULL OR minimum_due_minor BETWEEN 0 AND 1000000000000),
    statement_balance_minor INTEGER CHECK (statement_balance_minor IS NULL OR statement_balance_minor BETWEEN 0 AND 1000000000000),
    last_statement_issue_date TEXT CHECK (last_statement_issue_date IS NULL OR last_statement_issue_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    next_closing_date TEXT CHECK (next_closing_date IS NULL OR next_closing_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    retrieved_at TEXT NOT NULL CHECK (length(retrieved_at) = 20),
    provenance TEXT NOT NULL CHECK (provenance IN ('provider', 'manual', 'demo')),
    source_connection_id TEXT REFERENCES connections(id) ON DELETE SET NULL
) STRICT;

CREATE INDEX payment_snapshots_account_idx ON payment_snapshots (account_id, retrieved_at DESC);

CREATE TABLE manual_payment_metadata (
    account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    statement_balance_minor INTEGER CHECK (statement_balance_minor IS NULL OR statement_balance_minor BETWEEN 0 AND 1000000000000),
    minimum_due_minor INTEGER CHECK (minimum_due_minor IS NULL OR minimum_due_minor BETWEEN 0 AND 1000000000000),
    next_due_date TEXT CHECK (next_due_date IS NULL OR next_due_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    valid_through_date TEXT CHECK (valid_through_date IS NULL OR valid_through_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    updated_at TEXT NOT NULL CHECK (length(updated_at) = 20),
    CHECK (statement_balance_minor IS NOT NULL OR minimum_due_minor IS NOT NULL OR next_due_date IS NOT NULL)
) STRICT;

CREATE TABLE import_batches (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    file_sha256 TEXT NOT NULL CHECK (length(file_sha256) = 64),
    file_size_bytes INTEGER NOT NULL CHECK (file_size_bytes >= 0),
    mapping_json TEXT NOT NULL CHECK (length(mapping_json) <= 8192),
    row_count INTEGER NOT NULL DEFAULT 0 CHECK (row_count >= 0),
    inserted_count INTEGER NOT NULL DEFAULT 0 CHECK (inserted_count >= 0),
    duplicate_count INTEGER NOT NULL DEFAULT 0 CHECK (duplicate_count >= 0),
    review_count INTEGER NOT NULL DEFAULT 0 CHECK (review_count >= 0),
    rejected_count INTEGER NOT NULL DEFAULT 0 CHECK (rejected_count >= 0),
    status TEXT NOT NULL CHECK (status IN ('preview', 'imported', 'rolled_back', 'failed')),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    imported_at TEXT CHECK (imported_at IS NULL OR length(imported_at) = 20),
    rolled_back_at TEXT CHECK (rolled_back_at IS NULL OR length(rolled_back_at) = 20)
) STRICT;

CREATE INDEX import_batches_account_idx ON import_batches (account_id, created_at DESC);

CREATE TABLE transactions (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('manual', 'csv', 'provider', 'demo')),
    provider_transaction_id TEXT CHECK (provider_transaction_id IS NULL OR length(provider_transaction_id) BETWEEN 1 AND 200),
    import_batch_id TEXT REFERENCES import_batches(id) ON DELETE SET NULL,
    source_row_ref TEXT CHECK (source_row_ref IS NULL OR length(source_row_ref) <= 128),
    pending INTEGER NOT NULL DEFAULT 0 CHECK (pending IN (0, 1)),
    pending_link_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
    authorized_date TEXT CHECK (authorized_date IS NULL OR authorized_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    posted_date TEXT CHECK (posted_date IS NULL OR posted_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    reporting_date TEXT NOT NULL CHECK (reporting_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    reporting_date_source TEXT NOT NULL CHECK (reporting_date_source IN ('authorized', 'posted', 'manual')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor BETWEEN -1000000000000 AND 1000000000000),
    currency TEXT NOT NULL CHECK (length(currency) = 3 AND currency = upper(currency)),
    raw_description TEXT NOT NULL CHECK (length(raw_description) BETWEEN 1 AND 500),
    merchant_normalized TEXT NOT NULL CHECK (length(merchant_normalized) BETWEEN 1 AND 200),
    merchant_display TEXT NOT NULL CHECK (length(merchant_display) BETWEEN 1 AND 200),
    category TEXT CHECK (category IS NULL OR length(category) BETWEEN 1 AND 80),
    tx_type TEXT NOT NULL CHECK (tx_type IN ('purchase', 'refund', 'fee', 'interest', 'payment', 'transfer', 'other')),
    excluded INTEGER NOT NULL DEFAULT 0 CHECK (excluded IN (0, 1)),
    removed INTEGER NOT NULL DEFAULT 0 CHECK (removed IN (0, 1)),
    source_updated_at TEXT CHECK (source_updated_at IS NULL OR length(source_updated_at) = 20),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    updated_at TEXT NOT NULL CHECK (length(updated_at) = 20),
    CHECK (reporting_date_source <> 'authorized' OR authorized_date IS NOT NULL),
    CHECK (reporting_date_source <> 'posted' OR posted_date IS NOT NULL),
    CHECK (pending = 0 OR posted_date IS NULL OR posted_date >= reporting_date OR authorized_date IS NOT NULL)
) STRICT;

CREATE UNIQUE INDEX transactions_provider_identity_idx
    ON transactions (account_id, provider_transaction_id)
    WHERE provider_transaction_id IS NOT NULL;
CREATE INDEX transactions_account_date_idx ON transactions (account_id, reporting_date);
CREATE INDEX transactions_reporting_date_idx ON transactions (reporting_date, pending);
CREATE INDEX transactions_merchant_idx ON transactions (merchant_display);
CREATE INDEX transactions_batch_idx ON transactions (import_batch_id);

CREATE TABLE transaction_overrides (
    transaction_id TEXT PRIMARY KEY REFERENCES transactions(id) ON DELETE CASCADE,
    merchant_alias TEXT CHECK (merchant_alias IS NULL OR length(merchant_alias) BETWEEN 1 AND 120),
    category TEXT CHECK (category IS NULL OR length(category) BETWEEN 1 AND 80),
    type_correction TEXT CHECK (type_correction IS NULL OR type_correction IN ('purchase', 'refund', 'fee', 'interest', 'payment', 'transfer', 'other')),
    excluded INTEGER CHECK (excluded IS NULL OR excluded IN (0, 1)),
    notes TEXT CHECK (notes IS NULL OR length(notes) <= 2000),
    edited_at TEXT NOT NULL CHECK (length(edited_at) = 20)
) STRICT;

CREATE TABLE merchant_rules (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    match_type TEXT NOT NULL CHECK (match_type IN ('exact', 'contains', 'starts_with')),
    match_text TEXT NOT NULL CHECK (length(match_text) BETWEEN 1 AND 120),
    canonical_merchant TEXT NOT NULL CHECK (length(canonical_merchant) BETWEEN 1 AND 120),
    category TEXT CHECK (category IS NULL OR length(category) BETWEEN 1 AND 80),
    priority INTEGER NOT NULL DEFAULT 100 CHECK (priority BETWEEN 0 AND 10000),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    updated_at TEXT NOT NULL CHECK (length(updated_at) = 20),
    UNIQUE (match_type, match_text)
) STRICT;

CREATE INDEX merchant_rules_priority_idx ON merchant_rules (enabled, priority, id);

CREATE TABLE import_rows (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    batch_id TEXT NOT NULL REFERENCES import_batches(id) ON DELETE CASCADE,
    transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
    row_number INTEGER NOT NULL CHECK (row_number >= 1),
    source_row_hash TEXT NOT NULL CHECK (length(source_row_hash) = 64),
    fingerprint TEXT CHECK (fingerprint IS NULL OR length(fingerprint) = 64),
    status TEXT NOT NULL CHECK (status IN ('inserted', 'duplicate', 'review', 'rejected', 'rolled_back')),
    created_at TEXT NOT NULL CHECK (length(created_at) = 20),
    UNIQUE (batch_id, row_number)
) STRICT;

CREATE INDEX import_rows_batch_idx ON import_rows (batch_id, status);
CREATE INDEX import_rows_fingerprint_idx ON import_rows (fingerprint);

CREATE TABLE sync_runs (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    connection_id TEXT NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    trigger TEXT NOT NULL CHECK (trigger IN ('manual', 'scheduled', 'launch', 'resume', 'demo')),
    started_at TEXT NOT NULL CHECK (length(started_at) = 20),
    finished_at TEXT CHECK (finished_at IS NULL OR length(finished_at) = 20),
    status TEXT NOT NULL CHECK (status IN ('running', 'success', 'partial', 'failed', 'skipped')),
    error_code TEXT CHECK (error_code IS NULL OR length(error_code) <= 64),
    skip_reason TEXT CHECK (skip_reason IS NULL OR length(skip_reason) <= 128)
) STRICT;

CREATE INDEX sync_runs_connection_idx ON sync_runs (connection_id, started_at DESC);

CREATE TABLE sync_steps (
    id TEXT PRIMARY KEY CHECK (length(id) = 36),
    run_id TEXT NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    stream TEXT NOT NULL CHECK (stream IN ('transactions', 'balances', 'liabilities')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'success', 'partial', 'failed', 'skipped', 'unavailable')),
    items_added INTEGER NOT NULL DEFAULT 0 CHECK (items_added >= 0),
    items_modified INTEGER NOT NULL DEFAULT 0 CHECK (items_modified >= 0),
    items_removed INTEGER NOT NULL DEFAULT 0 CHECK (items_removed >= 0),
    provider_as_of TEXT CHECK (provider_as_of IS NULL OR length(provider_as_of) = 20),
    sanitized_error TEXT CHECK (sanitized_error IS NULL OR length(sanitized_error) <= 256),
    started_at TEXT NOT NULL CHECK (length(started_at) = 20),
    finished_at TEXT CHECK (finished_at IS NULL OR length(finished_at) = 20),
    UNIQUE (run_id, stream)
) STRICT;
