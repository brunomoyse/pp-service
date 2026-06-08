-- Locale of the device, captured at registration so push copy can be
-- localized per device (the achievement i18n strings live in the app, but
-- generic push copy is localized server-side from this column).
ALTER TABLE device_tokens ADD COLUMN locale VARCHAR(10);
