// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState, type FormEvent } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useT } from '../i18n'
import { api, useAction, useApi, useMe } from '../lib/api'
import type { Account, Project, Role } from '../lib/types'
import { ErrorNote, Loading, NotFound } from '../components/States'

interface Org {
  id: string
  slug: string
  display_name: string
  role: Role | null
  members?: { slug: string; role: Role }[]
}

interface Team {
  id: string
  slug: string
  name: string
  members: number
  projects: number
}

const ROLES: Role[] = ['viewer', 'reviewer', 'member', 'lead', 'admin']

export function Profile() {
  const t = useT()
  const { slug = '' } = useParams()
  const me = useMe()
  const account = useApi<Account>(`/api/accounts/${slug}`)
  const projects = useApi<Project[]>(`/api/accounts/${slug}/projects`)
  const isOrg = account.data?.kind === 'org'
  const org = useApi<Org>(isOrg ? `/api/orgs/${slug}` : null)
  const member = !!org.data?.members
  const teams = useApi<Team[]>(isOrg && member ? `/api/orgs/${slug}/teams` : null)
  const [teamSlug, setTeamSlug] = useState('')
  const [teamName, setTeamName] = useState('')
  const [who, setWho] = useState('')
  const [role, setRole] = useState<Role>('member')
  const [teamMember, setTeamMember] = useState('')
  const [teamForMember, setTeamForMember] = useState('')
  const [projectForTeam, setProjectForTeam] = useState('')
  const [teamForProject, setTeamForProject] = useState('')
  const [error, setError] = useState<string | null>(null)

  const invalidate = [[`/api/orgs/${slug}`], [`/api/orgs/${slug}/teams`], [`/api/accounts/${slug}/projects`]]
  const createTeam = useAction(
    () => api(`/api/orgs/${slug}/teams`, { method: 'POST', json: { slug: teamSlug, name: teamName } }),
    invalidate,
  )
  const addMember = useAction(
    () => api(`/api/orgs/${slug}/members/${who}`, { method: 'PUT', json: { role } }),
    invalidate,
  )
  const addTeamMember = useAction(
    () => api(`/api/orgs/${slug}/teams/${teamForMember}/members/${teamMember}`, { method: 'PUT' }),
    invalidate,
  )
  const grantTeam = useAction(
    () =>
      api(`/api/orgs/${slug}/teams/${teamForProject}/projects/${projectForTeam}`, {
        method: 'PUT',
        json: { role },
      }),
    invalidate,
  )

  if (account.isLoading) return <div className="page"><Loading /></div>
  if (account.error) {
    if (account.error.status === 404) return <NotFound />
    return <div className="page"><ErrorNote error={account.error} /></div>
  }
  if (!account.data) return <NotFound />
  const a = account.data
  const isSelf = me.data?.slug === slug
  const orgRole = org.data?.role ?? null
  const isAdmin = orgRole === 'admin'
  const canCreate = !isOrg && isSelf || orgRole === 'lead' || orgRole === 'admin'
  const initial = (a.display_name || a.slug).slice(0, 1)
  const fail = (e: { message: string }) => setError(e.message)
  const done = (fn: () => void) => (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    fn()
  }

  return (
    <main className="page">
      <header className="profile-header">
        <span className="avatar avatar-xl">{initial}</span>
        <div className="profile-heading">
          <h1>{a.display_name || a.slug}</h1>
          <p className="muted small">
            @{a.slug}
            <span className="tag" style={{ marginLeft: '0.5rem' }}>{isOrg ? t('profile.org') : t('profile.person')}</span>
            {orgRole && <span className="tag" style={{ marginLeft: '0.35rem' }}>{t(`role.${orgRole}`)}</span>}
          </p>
        </div>
        <div className="profile-actions">
          {!isOrg && isSelf && (
            <Link className="btn" to="/settings/profile">
              {t('profile.edit')}
            </Link>
          )}
          {canCreate && (
            <Link className="btn btn-primary" to={`/new?owner=${encodeURIComponent(slug)}`}>
              {t('nav.newProject')}
            </Link>
          )}
        </div>
      </header>

      {isOrg && member && (
        <nav className="repo-tabs org-tabs" aria-label={slug}>
          <a href="#projects">{t('org.projects')}</a>
          <a href="#members">{t('org.members')}</a>
          <a href="#teams">{t('org.teams')}</a>
        </nav>
      )}
      {error && <p className="error" role="alert">{error}</p>}

      <section id="projects" style={{ marginBottom: '2.5rem' }}>
        <h2>{t('org.projects')}</h2>
        {projects.isLoading && <Loading />}
        <ErrorNote error={projects.error} />
        {projects.data?.length === 0 && <div className="empty"><p>{t('org.noProjects')}</p></div>}
        {!!projects.data?.length && (
          <table className="schedule">
            <thead>
              <tr>
                <th>{t('branches.name')}</th>
                <th>{t('project.description')}</th>
                <th>{t('project.visibility')}</th>
                <th>{t('settings.role')}</th>
              </tr>
            </thead>
            <tbody>
              {projects.data.map((p) => (
                <tr key={p.id}>
                  <td>
                    <Link to={`/${p.full_name}`}>
                      <strong>{p.slug}</strong>
                    </Link>
                  </td>
                  <td className="muted small">{p.description}</td>
                  <td><span className="tag">{p.visibility === 'public' ? t('dash.public') : t('dash.private')}</span></td>
                  <td><span className="tag">{p.role ? t(`role.${p.role}`) : ''}</span></td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      {isOrg && member && (
        <section id="members" style={{ marginBottom: '2.5rem' }}>
          <h2>{t('org.members')}</h2>
          <table className="schedule">
            <thead>
              <tr>
                <th>{t('auth.slug')}</th>
                <th style={{ width: '10rem' }}>{t('settings.role')}</th>
              </tr>
            </thead>
            <tbody>
              {org.data!.members!.map((m) => (
                <tr key={m.slug}>
                  <td><Link to={`/${m.slug}`}><code>{m.slug}</code></Link></td>
                  <td className="small">{t(`role.${m.role}`)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {isAdmin && (
            <form className="form-block stack" style={{ maxWidth: '44rem' }} onSubmit={done(() => addMember.mutate(undefined, { onError: fail }))}>
              <h3>{t('org.addMember')}</h3>
              <div className="row">
                <input value={who} onChange={(e) => setWho(e.target.value)} placeholder="user-name" required style={{ maxWidth: '14rem' }} />
                <select value={role} onChange={(e) => setRole(e.target.value as Role)} style={{ width: 'auto' }}>
                  {ROLES.map((r) => <option key={r} value={r}>{t(`role.${r}`)}</option>)}
                </select>
                <button className="btn btn-primary" disabled={addMember.isPending || !who}>{t('org.add')}</button>
              </div>
            </form>
          )}
        </section>
      )}

      {isOrg && member && (
        <section id="teams">
          <h2>{t('org.teams')}</h2>
          {teams.isLoading && <Loading />}
          <ErrorNote error={teams.error} />
          {teams.data?.length === 0 && <div className="empty"><p>{t('org.noTeams')}</p></div>}
          {!!teams.data?.length && (
            <table className="schedule">
              <thead>
                <tr>
                  <th>{t('branches.name')}</th>
                  <th>{t('org.membersCount')}</th>
                  <th>{t('org.projectsCount')}</th>
                </tr>
              </thead>
              <tbody>
                {teams.data.map((team) => (
                  <tr key={team.id}>
                    <td>
                      <strong>{team.name}</strong>
                      <div className="small muted"><code>{team.slug}</code></div>
                    </td>
                    <td className="num">{team.members}</td>
                    <td className="num">{team.projects}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {isAdmin && (
            <>
              <form className="form-block stack" style={{ maxWidth: '44rem' }} onSubmit={done(() => createTeam.mutate(undefined, { onError: fail }))}>
                <h3>{t('org.newTeam')}</h3>
                <div className="row">
                  <input value={teamSlug} onChange={(e) => setTeamSlug(e.target.value)} placeholder="design" required style={{ maxWidth: '10rem' }} />
                  <input value={teamName} onChange={(e) => setTeamName(e.target.value)} placeholder="Design team" style={{ maxWidth: '14rem' }} />
                  <button className="btn btn-primary" disabled={createTeam.isPending || !teamSlug}>{t('org.create')}</button>
                </div>
              </form>
              <form className="form-block stack" style={{ maxWidth: '44rem' }} onSubmit={done(() => addTeamMember.mutate(undefined, { onError: fail }))}>
                <h3>{t('org.addTeamMember')}</h3>
                <div className="row">
                  <select value={teamForMember} onChange={(e) => setTeamForMember(e.target.value)} required style={{ maxWidth: '12rem' }}>
                    <option value="">{t('org.team')}</option>
                    {(teams.data ?? []).map((team) => <option key={team.id} value={team.slug}>{team.name}</option>)}
                  </select>
                  <input value={teamMember} onChange={(e) => setTeamMember(e.target.value)} placeholder="user-name" required style={{ maxWidth: '14rem' }} />
                  <button className="btn" disabled={addTeamMember.isPending || !teamForMember || !teamMember}>{t('org.add')}</button>
                </div>
              </form>
              <form className="form-block stack" style={{ maxWidth: '44rem' }} onSubmit={done(() => grantTeam.mutate(undefined, { onError: fail }))}>
                <h3>{t('org.grant')}</h3>
                <div className="row">
                  <select value={teamForProject} onChange={(e) => setTeamForProject(e.target.value)} required style={{ maxWidth: '12rem' }}>
                    <option value="">{t('org.team')}</option>
                    {(teams.data ?? []).map((team) => <option key={team.id} value={team.slug}>{team.name}</option>)}
                  </select>
                  <select value={projectForTeam} onChange={(e) => setProjectForTeam(e.target.value)} required style={{ maxWidth: '12rem' }}>
                    <option value="">{t('org.project')}</option>
                    {(projects.data ?? []).map((p) => <option key={p.id} value={p.slug}>{p.slug}</option>)}
                  </select>
                  <select value={role} onChange={(e) => setRole(e.target.value as Role)} style={{ maxWidth: '9rem' }}>
                    {ROLES.map((r) => <option key={r} value={r}>{t(`role.${r}`)}</option>)}
                  </select>
                  <button className="btn" disabled={grantTeam.isPending || !teamForProject || !projectForTeam}>{t('org.grantAction')}</button>
                </div>
              </form>
            </>
          )}
        </section>
      )}

      {!isOrg && isSelf && !!me.data?.orgs.length && (
        <section>
          <h2>{t('account.orgs')}</h2>
          <ul className="list">
            {me.data.orgs.map((o) => (
              <li key={o.slug} className="row" style={{ justifyContent: 'space-between' }}>
                <span>
                  <Link to={`/${o.slug}`}>{o.display_name}</Link>{' '}
                  <span className="muted small">{o.slug}</span>
                </span>
                <span className="tag">{t(`role.${o.role}` as 'role.admin')}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  )
}
