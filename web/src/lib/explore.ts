// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Project } from './types'

export type OwnerFilter = 'all' | 'user' | 'org'
export type ProjectSort = 'newest' | 'name' | 'owner'

/** Client-side filter for Explore: one search box, owner kind and a stable sort. */
export function filterProjects(projects: Project[], query: string, owner: OwnerFilter, sort: ProjectSort): Project[] {
  const q = query.trim().toLowerCase()
  const filtered = projects.filter((p) => {
    if (owner !== 'all' && p.owner_kind !== owner) return false
    if (!q) return true
    return `${p.full_name} ${p.owner} ${p.description}`.toLowerCase().includes(q)
  })
  return filtered.sort((a, b) => {
    if (sort === 'name') return a.slug.localeCompare(b.slug) || a.owner.localeCompare(b.owner)
    if (sort === 'owner') return a.owner.localeCompare(b.owner) || a.slug.localeCompare(b.slug)
    return b.created_at.localeCompare(a.created_at) || a.full_name.localeCompare(b.full_name)
  })
}
