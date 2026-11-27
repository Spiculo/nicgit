// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import { useApi } from '../lib/api'
import { whenIso } from '../lib/format'
import { filterProjects, type OwnerFilter, type ProjectSort } from '../lib/explore'
import type { Project } from '../lib/types'
import { ErrorNote, Loading } from '../components/States'

export function Explore() {
  const t = useT()
  const lang = useLang()
  const projects = useApi<Project[]>('/api/explore')
  const [query, setQuery] = useState('')
  const [owner, setOwner] = useState<OwnerFilter>('all')
  const [sort, setSort] = useState<ProjectSort>('newest')
  const list = projects.data ? filterProjects(projects.data, query, owner, sort) : []

  return (
    <main className="page">
      <header className="page-head">
        <h1>{t('nav.explore')}</h1>
        {!!projects.data?.length && (
          <span className="small muted">
            {list.length === 1 ? t('explore.resultOne') : t('explore.results', { n: list.length })}
          </span>
        )}
      </header>
      <div className="explore-bar">
        <input
          type="search"
          aria-label={t('explore.search')}
          placeholder={t('explore.search')}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <select aria-label={t('explore.owner')} value={owner} onChange={(e) => setOwner(e.target.value as OwnerFilter)}>
          <option value="all">{t('explore.allOwners')}</option>
          <option value="user">{t('explore.people')}</option>
          <option value="org">{t('explore.organisations')}</option>
        </select>
        <select aria-label={t('explore.sortNewest')} value={sort} onChange={(e) => setSort(e.target.value as ProjectSort)}>
          <option value="newest">{t('explore.sortNewest')}</option>
          <option value="name">{t('explore.sortName')}</option>
          <option value="owner">{t('explore.sortOwner')}</option>
        </select>
      </div>
      {projects.isLoading && <Loading />}
      <ErrorNote error={projects.error} />
      {projects.data?.length === 0 && (
        <div className="empty">
          <p>{t('explore.empty')}</p>
        </div>
      )}
      {!!projects.data?.length && list.length === 0 && (
        <div className="empty">
          <p>{t('explore.noResults')}</p>
        </div>
      )}
      {!!list.length && (
        <table className="schedule explore-table">
          <thead>
            <tr>
              <th>{t('project.name')}</th>
              <th>{t('project.description')}</th>
              <th>{t('explore.type')}</th>
              <th>{t('explore.created')}</th>
            </tr>
          </thead>
          <tbody>
            {list.map((p) => (
              <tr key={p.id}>
                <td>
                  <Link to={`/${p.full_name}`}>
                    <strong>{p.full_name}</strong>
                  </Link>
                </td>
                <td className="muted">{p.description}</td>
                <td>
                  <span className="tag">{p.owner_kind === 'org' ? t('explore.org') : t('explore.person')}</span>
                </td>
                <td className="small muted">{whenIso(p.created_at, lang)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </main>
  )
}
