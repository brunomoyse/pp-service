-- Self-serve onboarding lets a club register even when its VAT company does not
-- look like a non-profit (ASBL/VZW). Those clubs are accepted but flagged for
-- manual review. Non-profits verified via VIES are created with needs_review = false.

ALTER TABLE clubs ADD COLUMN needs_review BOOLEAN NOT NULL DEFAULT false;
