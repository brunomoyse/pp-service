-- Remove role constraint and index
ALTER TABLE users DROP CONSTRAINT users_role_check;
DROP INDEX users_role_idx;

-- Remove role column
ALTER TABLE users DROP COLUMN role;