// SPDX-License-Identifier: AGPL-3.0-or-later
import { Link } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import { useApi } from '../lib/api'
import { whenIso } from '../lib/format'
import type { Project } from '../lib/types'
import { ErrorNote, Loading } from '../components/States'

interface Notification {
  id: string
  kind: string
  data: Record<string, string | number>
  created_at: string
  read: boolean
}

function ProjectSchedule({ projects }: { projects: Project[] }) {
  const t = useT()
  return (
    <table className="schedule">
      <thead>
        <tr>
          <th>{t('project.name')}</th>
          <th>{t('project.description')}</th>
          <th>{t('project.visibility')}</th>
        </tr>
      </thead>
      <tbody>
        {projects.map((p) => (
          <tr key={p.id}>
            <td>
              <Link to={`/${p.full_name}`}>{p.full_name}</Link>
            </td>
            <td className="muted">{p.description}</td>
            <td>{p.visibility === 'public' ? t('dash.public') : t('dash.private')}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

export function Dashboard() {
  const t = useT()
  const lang = useLang()
  const projects = useApi<Project[]>('/api/projects')
  const inbox = useApi<{ reviews: { project: string; number: number; title: string }[]; notifications: Notification[] }>('/api/me/inbox')
  return (
    <main className="page">
      <div className="row" style={{ justifyContent: 'space-between', marginBottom: '1rem' }}>
        <h1 style={{ margin: 0 }}>{t('dash.title')}</h1>
        <div className="row">
          <Link className="btn" to="/import">
            {t('nav.import')}
          </Link>
          <Link className="btn btn-primary" to="/new">
            {t('nav.newProject')}
          </Link>
        </div>
      </div>
      {projects.isLoading && <Loading />}
      <ErrorNote error={projects.error} />
      {projects.data?.length === 0 && <p className="empty">{t('dash.empty')}</p>}
      {!!projects.data?.length && <ProjectSchedule projects={projects.data} />}

      {inbox.data && (
        <section style={{ marginTop: '2.5rem' }}>
          <h2>{t('dash.reviews')}</h2>
          {inbox.data.reviews.length === 0 ? (
            <p className="muted">{t('dash.noReviews')}</p>
          ) : (
            <ul className="list">
              {inbox.data.reviews.map((r) => (
                <li key={`${r.project}#${r.number}`}>
                  <Link to={`/${r.project}/proposals/${r.number}`}>
                    {r.project} #{r.number}: {r.title}
                  </Link>
                </li>
              ))}
            </ul>
          )}
          {inbox.data.notifications.length > 0 && (
            <>
              <h2 style={{ marginTop: '2rem' }}>{t('dash.notifications')}</h2>
              <ul className="list">
                {inbox.data.notifications.map((n) => (
                  <li key={n.id} className={n.read ? 'muted' : ''}>
                    <NotificationLine n={n} /> <span className="small muted">{whenIso(n.created_at, lang)}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>
      )}
    </main>
  )
}

function NotificationLine({ n }: { n: Notification }) {
  const d = n.data
  const project = String(d.project ?? '')
  switch (n.kind) {
    case 'proposal.review_requested':
    case 'proposal.reviewed':
    case 'proposal.combined':
      return <Link to={`/${project}/proposals/${d.number}`}>{`${project} #${d.number} ${d.title ?? ''}`}</Link>
    case 'issue.assigned':
      return <Link to={`/${project}/issues/${d.number}`}>{`${project} #${d.number} ${d.title ?? ''}`}</Link>
    default:
      return <Link to={`/${project}`}>{project}</Link>
  }
}

export function Explore() {
  const t = useT()
  const projects = useApi<Project[]>('/api/explore')
  return (
    <main className="page">
      <h1>{t('nav.explore')}</h1>
      {projects.isLoading && <Loading />}
      <ErrorNote error={projects.error} />
      {projects.data && <ProjectSchedule projects={projects.data} />}
    </main>
  )
}
