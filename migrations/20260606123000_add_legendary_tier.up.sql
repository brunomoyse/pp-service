-- Introduce the Legendary (holographic) rarity. The achievements.tier column is
-- a free VARCHAR (no CHECK), so no constraint change is needed — we just promote
-- the rarest existing achievement so Legendary is real and demonstrable.
UPDATE achievements
SET tier = 'legendary', updated_at = NOW()
WHERE code = 'first_win';
