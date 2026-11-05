-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Each claim of a job gets its own token, valid only while that claim runs (decision N30).
ALTER TABLE jobs ADD COLUMN token_hash text;
