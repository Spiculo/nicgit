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
