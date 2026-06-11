-- Track how each entry was paid, for the end-of-night cash report.
-- TEXT + CHECK mirrors the existing entry_type column (same idiom as
-- tournament_entries.entry_type) rather than a PG enum, so the set stays
-- easy to extend without an enum migration.
ALTER TABLE tournament_entries
    ADD COLUMN payment_method TEXT NOT NULL DEFAULT 'cash'
    CHECK (payment_method IN ('cash', 'card', 'bank_transfer', 'voucher', 'comp', 'other'));

CREATE INDEX idx_tournament_entries_payment_method
    ON tournament_entries (payment_method);
