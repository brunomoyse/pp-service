UPDATE achievements
SET tier = 'gold', updated_at = NOW()
WHERE code = 'first_win';
