-- SPDX-License-Identifier: AGPL-3.0-or-later
-- AI answers kept per subject (a proposal at given heads), so the router is asked once.
CREATE TABLE ai_summaries (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    subject    text NOT NULL,
    text       text NOT NULL,
    units      bigint NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, subject)
);
