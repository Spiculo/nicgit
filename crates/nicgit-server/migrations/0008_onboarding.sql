-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The first-run welcome: the answers that let NicGit choose its language and wording, and describe
-- the person's practice for later defaults. Existing accounts keep working and are marked as done.
ALTER TABLE users ADD COLUMN onboarded_at timestamptz;
ALTER TABLE users ADD COLUMN work text NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN years_experience integer CHECK (years_experience IS NULL OR (years_experience >= 0 AND years_experience <= 80));
ALTER TABLE users ADD COLUMN site_experience text NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN focus text NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN company text NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN team_size text NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN company_size text NOT NULL DEFAULT '';

UPDATE users SET onboarded_at = now();
