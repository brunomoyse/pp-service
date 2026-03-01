DROP TRIGGER IF EXISTS trg_blind_structure_templates_updated_at ON blind_structure_templates;
ALTER TABLE blind_structure_templates DROP COLUMN IF EXISTS updated_at;
