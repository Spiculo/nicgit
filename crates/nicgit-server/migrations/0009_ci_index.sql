-- SPDX-License-Identifier: AGPL-3.0-or-later
-- The CI page reads a project's checks runs, newest first (N63).
CREATE INDEX IF NOT EXISTS jobs_project_kind_created_idx ON jobs (project_id, kind, created_at DESC);
