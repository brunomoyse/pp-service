-- Restore table_name and location columns to club_tables
ALTER TABLE club_tables 
ADD COLUMN table_name TEXT,
ADD COLUMN location TEXT;