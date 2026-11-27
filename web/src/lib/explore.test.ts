// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, test } from 'vitest'
import { filterProjects } from './explore'
import type { Project } from './types'

const base = { description: '', visibility: 'public' as const, default_option: 'main', role: 'viewer' as const }
const projects: Project[] = [
  { ...base, id: '1', owner: 'studio', owner_kind: 'org', slug: 'house', full_name: 'studio/house', created_at: '2026-11-01T00:00:00Z' },
  { ...base, id: '2', owner: 'ana', owner_kind: 'user', slug: 'bridge', full_name: 'ana/bridge', description: 'A footbridge', created_at: '2026-11-20T00:00:00Z' },
  { ...base, id: '3', owner: 'studio', owner_kind: 'org', slug: 'school', full_name: 'studio/school', description: 'A school', created_at: '2026-11-10T00:00:00Z' },
]

describe('explore filters', () => {
  test('search matches name, owner and description', () => {
    expect(filterProjects(projects, 'bridge', 'all', 'newest').map((p) => p.slug)).toEqual(['bridge'])
    expect(filterProjects(projects, 'STUDIO', 'all', 'name').map((p) => p.slug)).toEqual(['house', 'school'])
    expect(filterProjects(projects, 'foot', 'all', 'newest').map((p) => p.slug)).toEqual(['bridge'])
  })

  test('owner kind filter', () => {
    expect(filterProjects(projects, '', 'user', 'name').map((p) => p.slug)).toEqual(['bridge'])
    expect(filterProjects(projects, '', 'org', 'name').map((p) => p.slug)).toEqual(['house', 'school'])
  })

  test('sort by newest, name and owner', () => {
    expect(filterProjects(projects, '', 'all', 'newest').map((p) => p.slug)).toEqual(['bridge', 'school', 'house'])
    expect(filterProjects(projects, '', 'all', 'name').map((p) => p.slug)).toEqual(['bridge', 'house', 'school'])
    expect(filterProjects(projects, '', 'all', 'owner').map((p) => p.slug)).toEqual(['bridge', 'house', 'school'])
  })

  test('no match gives an empty list', () => {
    expect(filterProjects(projects, 'nothing', 'all', 'newest')).toEqual([])
  })
})
