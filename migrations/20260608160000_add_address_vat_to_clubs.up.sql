-- Self-serve club onboarding captures the club's postal address and its
-- VAT / enterprise number (mandatory for non-profit orgs — ASBL/VZW and
-- equivalents — used as an anti-abuse gate, verified against VIES at signup).
-- Both columns stay nullable: pre-existing clubs were created without them.

ALTER TABLE clubs ADD COLUMN address    TEXT;
ALTER TABLE clubs ADD COLUMN vat_number TEXT;
