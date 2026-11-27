// SPDX-License-Identifier: AGPL-3.0-or-later
export type Role = 'viewer' | 'reviewer' | 'member' | 'lead' | 'admin'

export interface Project {
  id: string
  owner: string
  owner_kind: 'user' | 'org'
  slug: string
  full_name: string
  description: string
  visibility: 'private' | 'public'
  default_option: string
  created_at: string
  role: Role | null
}

export interface Summary {
  first: boolean
  added: number
  modified: number
  removed: number
  by_class: Record<string, { added: number; modified: number; removed: number }>
  by_level: Record<string, Record<string, { added: number; modified: number; removed: number }>>
  settings_changed: boolean
  other: number
  ai_text?: string
}

export interface Version {
  id: string
  parents: string[]
  author: string
  email: string
  time: number
  message: string
  summary: Summary | null
}

export interface OptionInfo {
  name: string
  label: string
  head: string
  time: number | null
  message: string | null
  author: string | null
  is_default: boolean
  is_agent: boolean
  protected: boolean
}

export interface Overview {
  project: Project
  role: Role
  options: OptionInfo[]
  empty: boolean
  open_proposals: number
  open_issues: number
  clone_url: string
  latest?: Version
  counts?: Record<string, number>
  schema?: string | null
  settings?: Record<string, unknown> | null
  milestones?: { name: string; target: string; message: string; tagger: string; time: number }[]
}

export interface Account {
  id: string
  slug: string
  kind: 'user' | 'org'
  display_name: string
}

export interface HomeRepo {
  id: string
  owner: string
  owner_kind: 'user' | 'org'
  slug: string
  full_name: string
  description: string
  visibility: 'private' | 'public'
  default_option: string
  created_at: string
  last_activity_at: string
  open_proposals: number
  open_issues: number
}

export interface HomePost {
  id: string
  kind: string
  data: Record<string, string | number>
  read: boolean
  created_at: string
}

export interface HomeRelease {
  project: string
  tag: string
  title: string
  phase: string
  commit: string
  created_by: string
  created_at: string
}

export interface HomeProposal {
  project: string
  number: number
  title: string
  source: string
  target: string
  author: string
  created_at: string
}

export interface HomeIssue {
  project: string
  number: number
  title: string
  priority: string
  author: string
  assignee: string | null
  created_at: string
}

export interface HomeRun {
  project: string
  id: string
  status: 'queued' | 'running' | 'done' | 'failed'
  progress: number
  result: { passed?: boolean } | null
  created_at: string
  finished_at: string | null
}

export interface HomeData {
  repos: HomeRepo[]
  posts: HomePost[]
  releases: HomeRelease[]
  packages: unknown[]
  proposals: HomeProposal[]
  issues: HomeIssue[]
  runs: HomeRun[]
}

export interface FieldChange {
  name: string
  kind: string
  old: unknown
  new: unknown
}

export interface ElementChange {
  guid: string
  type: string
  name: string
  kind: 'added' | 'modified' | 'removed'
  level: string
  attributes: FieldChange[]
  properties: { pset?: string; name: string; kind: string; old: unknown; new: unknown }[]
}

export interface CompareResult {
  details: ElementChange[]
  truncated: boolean
  categories: Record<string, { added: number; modified: number; removed: number }>
  tree: Record<string, Record<string, { added: number; modified: number; removed: number }>>
  from: string
  to: string
  other: number
  settings: string[]
}

export interface SettingsDiff {
  changed: { key: string; old: unknown; new: unknown }[]
}

export interface CiIssue {
  severity: string
  message: string
  guids?: string[]
}

export interface CiRunCheck {
  check: string
  title: string
  ok: boolean
  total: number
  counts: { error: number; warning: number; info: number }
  issues?: CiIssue[]
  truncated?: boolean
  seconds?: number
  failed: boolean
  error?: string
}

export interface CiRunResult {
  rev: string
  passed: boolean
  seconds?: number
  checks: CiRunCheck[]
}

export interface CiRunSummary {
  check: string
  title: string
  failed: boolean
  total: number
  counts: { error: number; warning: number; info: number }
  error?: string
}

export interface CiRun {
  id: string
  commit: string
  checks: string[]
  status: 'queued' | 'running' | 'done' | 'failed'
  progress: number
  passed: boolean | null
  seconds: number | null
  result: CiRunResult | null
  summary?: CiRunSummary[]
  error: string | null
  created_at: string
  started_at: string | null
  finished_at: string | null
  duration: number | null
}

export interface CiCheckOption {
  option: string
  head: string
  protected: boolean
  required: string[]
  approvals: number
  state: 'none' | 'pending' | 'passed' | 'failed'
  job: string | null
  result: unknown
}
