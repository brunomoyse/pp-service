ALTER TABLE blind_structure_templates
    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

UPDATE blind_structure_templates SET updated_at = created_at;

CREATE TRIGGER trg_blind_structure_templates_updated_at
    BEFORE UPDATE ON blind_structure_templates
    FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
