-- Migration 3: user edit/rule provenance and import fingerprinting.
-- Rule-derived display fields are separate from source fields so a rule change
-- can be reapplied without rewriting provider/CSV source data.

ALTER TABLE transactions ADD COLUMN rule_merchant TEXT;
ALTER TABLE transactions ADD COLUMN rule_category TEXT;
ALTER TABLE transactions ADD COLUMN rule_id TEXT;
ALTER TABLE transactions ADD COLUMN import_fingerprint TEXT;

CREATE INDEX transactions_rule_idx ON transactions (rule_id) WHERE rule_id IS NOT NULL;
CREATE INDEX transactions_import_fingerprint_idx ON transactions (account_id, import_fingerprint)
    WHERE import_fingerprint IS NOT NULL;
CREATE INDEX transactions_source_row_idx ON transactions (account_id, source_row_ref)
    WHERE source_row_ref IS NOT NULL;
