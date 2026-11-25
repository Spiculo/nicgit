// SPDX-License-Identifier: AGPL-3.0-or-later
import { createContext, useContext, useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { Link, Route, Routes, useLocation, useParams } from 'react-router-dom'
import { useT } from '../i18n'
import type { Key } from '../i18n/en'
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
import { Preview } from './Preview'
import { CIPage, RendersPage, SheetsPage, PackagesPage, AgentsPage } from './Soon'

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

type Tab = 'code' | 'pulls' | 'issues' | 'preview' | 'ci' | 'renders' | 'sheets' | 'packages' | 'agents' | 'configs'

const CODE_PATHS = ['', 'files', 'versions', 'branches', 'compare', 'milestones', 'activity']

/** The repo tab a pathname belongs to. Code owns its sub-pages. */
function tabOf(rest: string): Tab {
  const p = rest.replace(/\/+$/, '') || '/'
  if (p === '/' || CODE_PATHS.some((x) => x && (p === `/${x}` || p.startsWith(`/${x}/`)))) return 'code'
  if (p.startsWith('/proposals')) return 'pulls'
  if (p.startsWith('/issues')) return 'issues'
  if (p.startsWith('/preview')) return 'preview'
  if (p.startsWith('/ci')) return 'ci'
  if (p.startsWith('/renders')) return 'renders'
  if (p.startsWith('/sheets')) return 'sheets'
  if (p.startsWith('/packages')) return 'packages'
  if (p.startsWith('/agents')) return 'agents'
  if (p.startsWith('/settings')) return 'configs'
  return 'code'
}

const CODE_SUB = [
  { path: '', key: 'nav.overview' },
  { path: '/files', key: 'rail.files' },
  { path: '/versions', key: 'rail.versions' },
  { path: '/branches', key: 'rail.branches' },
  { path: '/compare', key: 'rail.compare' },
  { path: '/milestones', key: 'rail.milestones' },
  { path: '/activity', key: 'rail.activity' },
] as const

const CONFIG_SIDE = [
  { anchor: 'general', key: 'config.general' },
  { anchor: 'members', key: 'config.people' },
  { anchor: 'rules', key: 'config.rules' },
  { anchor: 'ai', key: 'config.ai' },
  { anchor: 'budgets', key: 'config.budgets' },
  { anchor: 'webhooks', key: 'config.webhooks' },
  { anchor: 'audit', key: 'config.audit' },
  { anchor: 'danger', key: 'config.danger' },
] as const

const SIDES: Partial<Record<Tab, readonly { anchor: string; key: string }[]>> = {
  preview: [
    { anchor: 'diff', key: 'preview.current' },
    { anchor: 'plan', key: 'preview.next' },
    { anchor: 'files', key: 'preview.files' },
  ],
  ci: [
    { anchor: 'runs', key: 'ci.runs' },
    { anchor: 'checks', key: 'ci.checks' },
    { anchor: 'schedules', key: 'ci.schedules' },
    { anchor: 'budgets', key: 'ci.budgets' },
  ],
  renders: [
    { anchor: 'jobs', key: 'renders.jobs' },
    { anchor: 'presets', key: 'renders.presets' },
    { anchor: 'usage', key: 'renders.usage' },
  ],
  sheets: [
    { anchor: 'list', key: 'sheets.list' },
    { anchor: 'pdf', key: 'sheets.pdf' },
    { anchor: 'revisions', key: 'sheets.revisions' },
  ],
  packages: [
    { anchor: 'objects', key: 'packages.objects' },
    { anchor: 'libraries', key: 'packages.libraries' },
    { anchor: 'rules', key: 'packages.rules' },
  ],
  agents: [
    { anchor: 'sessions', key: 'agents.sessions' },
    { anchor: 'tools', key: 'agents.tools' },
    { anchor: 'approvals', key: 'agents.approvals' },
  ],
  configs: CONFIG_SIDE,
}

export function ProjectLayout() {
  const t = useT()
  const { owner = '', slug = '' } = useParams()
  const { pathname } = useLocation()
  const overview = useApi<OverviewData>(projectApi(owner, slug, '/overview'))
  if (overview.isLoading) return <div className="page"><Loading /></div>
  if (overview.error) return <div className="page"><ErrorNote error={overview.error} /></div>
  if (!overview.data) return <NotFound />
  const role = overview.data.role
  const base = `/${owner}/${slug}`
  const rest = pathname.startsWith(base) ? pathname.slice(base.length) || '/' : pathname
  const active = tabOf(rest)
  const ctx: ProjectCtx = {
    owner,
    slug,
    base,
    api: (r = '') => projectApi(owner, slug, r),
    overview: overview.data,
    role,
    can: (need) => ORDER.indexOf(role) >= ORDER.indexOf(need),
  }
  const side = SIDES[active]
  const tabs: { id: Tab; path: string; key: string; count?: number; admin?: boolean }[] = [
    { id: 'code', path: '', key: 'nav.code' },
    { id: 'pulls', path: '/proposals', key: 'nav.pulls', count: overview.data.open_proposals },
    { id: 'issues', path: '/issues', key: 'nav.issues', count: overview.data.open_issues },
    { id: 'preview', path: '/preview', key: 'nav.preview' },
    { id: 'ci', path: '/ci', key: 'nav.ci' },
    { id: 'renders', path: '/renders', key: 'nav.renders' },
    { id: 'sheets', path: '/sheets', key: 'nav.sheets' },
    { id: 'packages', path: '/packages', key: 'nav.packages' },
    { id: 'agents', path: '/agents', key: 'nav.agents' },
    { id: 'configs', path: '/settings', key: 'nav.configs', admin: true },
  ]
  const subActive = (path: string) => {
    const p = rest.replace(/\/+$/, '') || '/'
    return path === '' ? p === '/' : p === path || p.startsWith(`${path}/`)
  }
  return (
    <Ctx.Provider value={ctx}>
      <Crumbs owner={owner} slug={slug} />
      <div className="project">
        <nav className="repo-tabs" aria-label={overview.data.project.full_name}>
          {tabs
            .filter((tab) => !tab.admin || ctx.can('admin'))
            .map((tab) => (
              <Link key={tab.id} to={`${base}${tab.path}`} className={active === tab.id ? 'active' : ''}>
                <span>{t(tab.key as Key)}</span>
                {!!tab.count && <span className="count">{tab.count}</span>}
              </Link>
            ))}
        </nav>
        {active === 'code' && (
          <nav className="repo-subnav" aria-label={t('nav.code')}>
            {CODE_SUB.map((item) => (
              <Link key={item.path} to={`${base}${item.path}`} className={subActive(item.path) ? 'active' : ''}>
                {t(item.key as Key)}
              </Link>
            ))}
          </nav>
        )}
        <div className={`repo-body${side ? ' with-side' : ''}`}>
          {side && (
            <aside className="repo-side" aria-label={t('nav.sections')}>
              {side.map((item) => (
                <a key={item.anchor} href={`#${item.anchor}`}>
                  {t(item.key as Key)}
                </a>
              ))}
            </aside>
          )}
          <div className="repo-main">
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
              <Route path="preview" element={<Preview />} />
              <Route path="ci" element={<CIPage />} />
              <Route path="renders" element={<RendersPage />} />
              <Route path="sheets" element={<SheetsPage />} />
              <Route path="packages" element={<PackagesPage />} />
              <Route path="agents" element={<AgentsPage />} />
              <Route path="settings" element={<ProjectSettings />} />
              <Route path="*" element={<NotFound />} />
            </Routes>
          </div>
        </div>
      </div>
    </Ctx.Provider>
  )
}
