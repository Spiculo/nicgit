-- SPDX-License-Identifier: AGPL-3.0-or-later
-- NicGit schema v1 (spec sections 7, 8, 12).

CREATE TABLE accounts (
    id           uuid PRIMARY KEY,
    kind         text NOT NULL CHECK (kind IN ('user', 'org')),
    slug         text NOT NULL UNIQUE,
    display_name text NOT NULL DEFAULT '',
    created_at   timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE users (
    account_id    uuid PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    email         text NOT NULL UNIQUE,
    password_hash text NOT NULL,
    is_site_admin boolean NOT NULL DEFAULT false,
    lang          text NOT NULL DEFAULT 'en'
);

CREATE TABLE sessions (
    id         text PRIMARY KEY,           -- sha256 of the cookie token
    user_id    uuid NOT NULL REFERENCES users(account_id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE tokens (
    id           uuid PRIMARY KEY,
    user_id      uuid NOT NULL REFERENCES users(account_id) ON DELETE CASCADE,
    name         text NOT NULL,
    token_hash   text NOT NULL UNIQUE,
    last_used_at timestamptz,
    created_at   timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE org_members (
    org_id  uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(account_id) ON DELETE CASCADE,
    role    text NOT NULL,
    PRIMARY KEY (org_id, user_id)
);

CREATE TABLE teams (
    id     uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    slug   text NOT NULL,
    name   text NOT NULL DEFAULT '',
    UNIQUE (org_id, slug)
);

CREATE TABLE team_members (
    team_id uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(account_id) ON DELETE CASCADE,
    PRIMARY KEY (team_id, user_id)
);

CREATE TABLE projects (
    id             uuid PRIMARY KEY,
    owner_id       uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    slug           text NOT NULL,
    description    text NOT NULL DEFAULT '',
    visibility     text NOT NULL DEFAULT 'private' CHECK (visibility IN ('private', 'public')),
    default_option text NOT NULL DEFAULT 'main',
    created_at     timestamptz NOT NULL DEFAULT now(),
    UNIQUE (owner_id, slug)
);

CREATE TABLE project_members (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id    uuid NOT NULL REFERENCES users(account_id) ON DELETE CASCADE,
    role       text NOT NULL,
    PRIMARY KEY (project_id, user_id)
);

CREATE TABLE team_project_access (
    team_id    uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role       text NOT NULL,
    PRIMARY KEY (team_id, project_id)
);

CREATE TABLE option_rules (
    project_id         uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    option             text NOT NULL,
    required_approvals integer NOT NULL DEFAULT 1,
    required_roles     text[] NOT NULL DEFAULT '{}',
    allowed_modes      text[] NOT NULL DEFAULT '{auto,whole:ours,whole:theirs,granular}',
    required_checks    text[] NOT NULL DEFAULT '{}',
    protected_settings text[] NOT NULL DEFAULT '{}',
    dismiss_on_push    boolean NOT NULL DEFAULT true,
    PRIMARY KEY (project_id, option)
);

CREATE TABLE proposals (
    id           uuid PRIMARY KEY,
    project_id   uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number       integer NOT NULL,
    title        text NOT NULL,
    body         text NOT NULL DEFAULT '',
    source       text NOT NULL,
    target       text NOT NULL,
    author_id    uuid NOT NULL REFERENCES accounts(id),
    state        text NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'combined', 'closed')),
    source_head  text NOT NULL,
    target_head  text NOT NULL,
    merge_commit text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, number)
);

CREATE TABLE reviews (
    id          uuid PRIMARY KEY,
    proposal_id uuid NOT NULL REFERENCES proposals(id) ON DELETE CASCADE,
    user_id     uuid NOT NULL REFERENCES accounts(id),
    verdict     text NOT NULL CHECK (verdict IN ('approve', 'changes', 'comment')),
    body        text NOT NULL DEFAULT '',
    source_head text NOT NULL,
    dismissed   boolean NOT NULL DEFAULT false,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE issues (
    id          uuid PRIMARY KEY,
    project_id  uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number      integer NOT NULL,
    title       text NOT NULL,
    body        text NOT NULL DEFAULT '',
    state       text NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'closed')),
    priority    text NOT NULL DEFAULT 'normal',
    author_id   uuid NOT NULL REFERENCES accounts(id),
    assignee_id uuid REFERENCES accounts(id),
    labels      text[] NOT NULL DEFAULT '{}',
    viewpoint   jsonb,
    elements    text[] NOT NULL DEFAULT '{}',
    version     text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, number)
);

CREATE TABLE comments (
    id          uuid PRIMARY KEY,
    project_id  uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    proposal_id uuid REFERENCES proposals(id) ON DELETE CASCADE,
    issue_id    uuid REFERENCES issues(id) ON DELETE CASCADE,
    author_id   uuid NOT NULL REFERENCES accounts(id),
    body        text NOT NULL,
    pin         jsonb,
    version     text,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE milestones (
    id            uuid PRIMARY KEY,
    project_id    uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    phase         text NOT NULL,
    tag           text NOT NULL,
    commit_id     text NOT NULL,
    title         text NOT NULL,
    notes         text NOT NULL DEFAULT '',
    created_by    uuid NOT NULL REFERENCES accounts(id),
    signed_off_by uuid REFERENCES accounts(id),
    signed_off_at timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, tag)
);

CREATE TABLE jobs (
    id          uuid PRIMARY KEY,
    project_id  uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind        text NOT NULL,
    args        jsonb NOT NULL DEFAULT '{}',
    cache_key   text UNIQUE,
    status      text NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'running', 'done', 'failed')),
    progress    real NOT NULL DEFAULT 0,
    result      jsonb,
    error       text,
    attempts    integer NOT NULL DEFAULT 0,
    created_at  timestamptz NOT NULL DEFAULT now(),
    started_at  timestamptz,
    finished_at timestamptz
);
CREATE INDEX jobs_queued ON jobs (created_at) WHERE status = 'queued';

CREATE TABLE activity (
    id         bigserial PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor_id   uuid REFERENCES accounts(id),
    kind       text NOT NULL,
    data       jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX activity_project ON activity (project_id, id DESC);

CREATE TABLE audit_log (
    id         bigserial PRIMARY KEY,
    account_id uuid,                       -- whose data (user or org); kept after deletion
    actor_id   uuid,
    action     text NOT NULL,
    target     text NOT NULL DEFAULT '',
    data       jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE usage_events (
    id         bigserial PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    project_id uuid,
    kind       text NOT NULL,
    quantity   bigint NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX usage_account ON usage_events (account_id, kind, created_at);

CREATE TABLE budgets (
    account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    kind       text NOT NULL,
    soft_limit bigint,
    hard_limit bigint,
    PRIMARY KEY (account_id, kind)
);

CREATE TABLE ai_settings (
    account_id uuid PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    enabled    boolean NOT NULL DEFAULT false
);

CREATE TABLE notifications (
    id         uuid PRIMARY KEY,
    user_id    uuid NOT NULL REFERENCES users(account_id) ON DELETE CASCADE,
    kind       text NOT NULL,
    data       jsonb NOT NULL DEFAULT '{}',
    read       boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX notifications_user ON notifications (user_id, created_at DESC);
