-- SPDX-License-Identifier: AGPL-3.0-or-later
-- A user name given up by a rename stays out of use, so nobody can take it and receive what was meant for its
-- old owner (decision N34). Only the account that gave it up may take it back.
CREATE TABLE retired_slugs (
    slug text PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    retired_at timestamptz NOT NULL DEFAULT now()
);

-- Picking the next job looks at how many each account has running (decision N33).
CREATE INDEX jobs_running_project_idx ON jobs (project_id) WHERE status = 'running';
