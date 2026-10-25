-- Migration 4: separate live provider snapshots from snapshots that arrived in
-- a restore. After any restore, restored provider balances, available credit,
-- limits, and due metadata are hidden until the connection produces fresh live
-- data. Restored transaction history is unaffected.

ALTER TABLE balance_snapshots ADD COLUMN restored INTEGER NOT NULL DEFAULT 0
    CHECK (restored IN (0, 1));
ALTER TABLE payment_snapshots ADD COLUMN restored INTEGER NOT NULL DEFAULT 0
    CHECK (restored IN (0, 1));

CREATE INDEX balance_snapshots_live_idx
    ON balance_snapshots (account_id, provenance, restored, retrieved_at DESC);
CREATE INDEX payment_snapshots_live_idx
    ON payment_snapshots (account_id, provenance, restored, retrieved_at DESC);
