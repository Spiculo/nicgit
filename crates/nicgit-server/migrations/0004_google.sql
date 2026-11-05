-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Google is the only sign-in (decision N28). The account's identity is Google's subject id, not its email.
ALTER TABLE users ADD COLUMN google_sub text UNIQUE;
ALTER TABLE users DROP COLUMN password_hash;
