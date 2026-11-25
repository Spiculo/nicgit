// SPDX-License-Identifier: AGPL-3.0-or-later
import { createContext, useContext, useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { Link, NavLink, Route, Routes, useParams } from 'react-router-dom'
import { useT } from '../i18n'
import { projectApi, useApi } from '../lib/api'
import type { Overview as OverviewData, Role } from '../lib/types'
import { ErrorNote, Loading, NotFound } from '../components/States'
import { Overview } from './Overview'
import { Versions } from './Versions'
import { Compare } from './Compare'
import { Files } from './Files'
import { Activity } from './Activity'
import { ProjectSettings } from './ProjectSettings'
import { Proposals, NewProposal, ProposalDetail } from './Proposals'
import { Issues, NewIssue, IssueDetail } from './Issues'
import { Milestones } from './Milestones'
import { Branches } from './Branches'

export interface ProjectCtx {
  owner: string
  slug: string
  base: string
  api: (rest?: string) => string
  overview: OverviewData
  role: Role
  can: (need: Role) => boolean
}

const Ctx = createContext<ProjectCtx | null>(null)

export function useProject(): ProjectCtx {
  const c = useContext(Ctx)
  if (!c) throw new Error('useProject outside a project')
  return c
}

const ORDER: Role[] = ['viewer', 'reviewer', 'member', 'lead', 'admin']

function Crumbs({ owner, slug }: { owner: string; slug: string }) {
  const [el, setEl] = useState<HTMLElement | null>(null)
  useEffect(() => setEl(document.getElementById('crumbs')), [])
  if (!el) return null
  return createPortal(
    <>
      <Link to="/">{owner}</Link>
      <span className="sep">/</span>
      <Link to={`/${owner}/${slug}`}>{slug}</Link>
    </>,
    el,
  )
}

export function ProjectLayout() {
  const t = useT()
  const { owner = '', slug = '' } = useParams()
  const overview = useApi<OverviewData>(projectApi(owner, slug, '/overview'))
  if (overview.isLoading) return <div className="page"><Loading /></div>
  if (overview.error) return <div className="page"><ErrorNote error={overview.error} /></div>
  if (!overview.data) return <NotFound />
  const role = overview.data.role
  const ctx: ProjectCtx = {
    owner,
    slug,
    base: `/${owner}/${slug}`,
    api: (rest = '') => projectApi(owner, slug, rest),
    overview: overview.data,
    role,
    can: (need) => ORDER.indexOf(role) >= ORDER.indexOf(need),
  }
  const link = (to: string, label: string, count?: number) => (
    <NavLink to={`${ctx.base}${to}`} end={to === ''} className={({ isActive }) => (isActive ? 'active' : '')}>
      <span>{label}</span>
      {!!count && <span className="count">{count}</span>}
    </NavLink>
  )
  return (
    <Ctx.Provider value={ctx}>
      <Crumbs owner={owner} slug={slug} />
      <div className="project">
        <nav className="rail" aria-label={overview.data.project.full_name}>
          {link('', t('rail.model'))}
          {link('/versions', t('rail.versions'))}
          {link('/branches', t('rail.branches'))}
          {link('/compare', t('rail.compare'))}
          {link('/proposals', t('rail.proposals'), overview.data.open_proposals)}
          {link('/issues', t('rail.issues'), overview.data.open_issues)}
          {link('/milestones', t('rail.milestones'))}
          {link('/files', t('rail.files'))}
          {link('/activity', t('rail.activity'))}
          {ctx.can('admin') && link('/settings', t('rail.settings'))}
        </nav>
        <div className="project-main">
          <Routes>
            <Route index element={<Overview />} />
            <Route path="versions" element={<Versions />} />
            <Route path="branches" element={<Branches />} />
            <Route path="compare" element={<Compare />} />
            <Route path="proposals" element={<Proposals />} />
            <Route path="proposals/new" element={<NewProposal />} />
            <Route path="proposals/:number" element={<ProposalDetail />} />
            <Route path="issues" element={<Issues />} />
            <Route path="issues/new" element={<NewIssue />} />
            <Route path="issues/:number" element={<IssueDetail />} />
            <Route path="milestones" element={<Milestones />} />
            <Route path="files" element={<Files />} />
            <Route path="activity" element={<Activity />} />
            <Route path="settings" element={<ProjectSettings />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
        </div>
      </div>
    </Ctx.Provider>
  )
}
