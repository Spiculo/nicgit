-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Workers report every minute while a job runs; only a job whose worker went silent is handed out again.
ALTER TABLE jobs ADD COLUMN heartbeat_at timestamptz;
