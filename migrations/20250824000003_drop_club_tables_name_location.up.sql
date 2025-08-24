-- Drop table_name and location columns from club_tables
ALTER TABLE club_tables 
DROP COLUMN IF EXISTS table_name,
DROP COLUMN IF EXISTS location;