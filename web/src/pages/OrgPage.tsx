// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState, type FormEvent } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useT } from '../i18n'
import { api, useAction, useApi } from '../lib/api'
import type { Project, Role } from '../lib/types'
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

export function OrgPage() {
  const t = useT()
  const { slug = '' } = useParams()
  const org = useApi<Org>(`/api/orgs/${slug}`)
  const projects = useApi<Project[]>(`/api/orgs/${slug}/projects`)
  const teams = useApi<Team[]>(`/api/orgs/${slug}/teams`)
  const [teamSlug, setTeamSlug] = useState('')
  const [teamName, setTeamName] = useState('')
  const [member, setMember] = useState('')
  const [role, setRole] = useState<Role>('member')
  const [teamMember, setTeamMember] = useState('')
  const [teamForMember, setTeamForMember] = useState('')
  const [projectForTeam, setProjectForTeam] = useState('')
  const [teamForProject, setTeamForProject] = useState('')
  const [error, setError] = useState<string | null>(null)

  const invalidate = [[`/api/orgs/${slug}`], [`/api/orgs/${slug}/teams`], [`/api/orgs/${slug}/projects`]]
  const createTeam = useAction(
    () => api(`/api/orgs/${slug}/teams`, { method: 'POST', json: { slug: teamSlug, name: teamName } }),
    invalidate,
  )
  const addMember = useAction(
    () => api(`/api/orgs/${slug}/members/${member}`, { method: 'PUT', json: { role } }),
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

  if (org.isLoading) return <div className="page"><Loading /></div>
  if (org.error) return <div className="page"><ErrorNote error={org.error} /></div>
  if (!org.data) return <NotFound />
  const admin = org.data.role === 'admin'
  const fail = (e: { message: string }) => setError(e.message)
  const done = (fn: () => void) => (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    fn()
  }

  return (
    <main className="page stack">
      <header className="row" style={{ justifyContent: 'space-between' }}>
        <div>
          <h1>{org.data.display_name || org.data.slug}</h1>
          <p className="muted">
            {org.data.slug} · {org.data.role}
          </p>
        </div>
        <Link className="btn" to="/">Dashboard</Link>
      </header>
      {error && <p className="error" role="alert">{error}</p>}

      <section>
        <h2>{t('org.projects')}</h2>
        {projects.isLoading && <Loading />}
        <ErrorNote error={projects.error} />
        {projects.data?.length === 0 && <p className="empty">{t('org.noProjects')}</p>}
        {!!projects.data?.length && (
          <table className="schedule">
            <thead>
              <tr>
                <th>{t('branches.name')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {projects.data.map((p) => (
                <tr key={p.id}>
                  <td>
                    <Link to={`/${p.owner}/${p.slug}`}>
                      <strong>{p.slug}</strong>
                    </Link>
                    <div className="small muted">{p.description}</div>
                  </td>
                  <td className="num small">{p.visibility}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <section>
        <h2>{t('org.members')}</h2>
        {!!org.data.members?.length && (
          <table className="schedule">
            <tbody>
              {org.data.members.map((m) => (
                <tr key={m.slug}>
                  <td><code>{m.slug}</code></td>
                  <td className="small">{m.role}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {admin && (
          <form className="card stack" onSubmit={done(() => addMember.mutate(undefined, { onError: fail }))}>
            <h3>{t('org.addMember')}</h3>
            <div className="row">
              <input value={member} onChange={(e) => setMember(e.target.value)} placeholder="user-name" required />
              <select value={role} onChange={(e) => setRole(e.target.value as Role)}>
                {ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
              </select>
              <button className="btn btn-primary" disabled={addMember.isPending || !member}>{t('org.add')}</button>
            </div>
          </form>
        )}
      </section>

      <section>
        <h2>{t('org.teams')}</h2>
        {teams.isLoading && <Loading />}
        <ErrorNote error={teams.error} />
        {teams.data?.length === 0 && <p className="empty">{t('org.noTeams')}</p>}
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
        {admin && (
          <>
            <form className="card stack" onSubmit={done(() => createTeam.mutate(undefined, { onError: fail }))}>
              <h3>{t('org.newTeam')}</h3>
              <div className="row">
                <input value={teamSlug} onChange={(e) => setTeamSlug(e.target.value)} placeholder="design" required />
                <input value={teamName} onChange={(e) => setTeamName(e.target.value)} placeholder="Design team" />
                <button className="btn btn-primary" disabled={createTeam.isPending || !teamSlug}>{t('org.create')}</button>
              </div>
            </form>
            <form className="card stack" onSubmit={done(() => addTeamMember.mutate(undefined, { onError: fail }))}>
              <h3>{t('org.addTeamMember')}</h3>
              <div className="row">
                <select value={teamForMember} onChange={(e) => setTeamForMember(e.target.value)} required>
                  <option value="">{t('org.team')}</option>
                  {(teams.data ?? []).map((team) => <option key={team.id} value={team.slug}>{team.name}</option>)}
                </select>
                <input value={teamMember} onChange={(e) => setTeamMember(e.target.value)} placeholder="user-name" required />
                <button className="btn" disabled={addTeamMember.isPending || !teamForMember || !teamMember}>{t('org.add')}</button>
              </div>
            </form>
            <form className="card stack" onSubmit={done(() => grantTeam.mutate(undefined, { onError: fail }))}>
              <h3>{t('org.grant')}</h3>
              <div className="row">
                <select value={teamForProject} onChange={(e) => setTeamForProject(e.target.value)} required>
                  <option value="">{t('org.team')}</option>
                  {(teams.data ?? []).map((team) => <option key={team.id} value={team.slug}>{team.name}</option>)}
                </select>
                <select value={projectForTeam} onChange={(e) => setProjectForTeam(e.target.value)} required>
                  <option value="">{t('org.project')}</option>
                  {(projects.data ?? []).map((p) => <option key={p.id} value={p.slug}>{p.slug}</option>)}
                </select>
                <select value={role} onChange={(e) => setRole(e.target.value as Role)}>
                  {ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
                </select>
                <button className="btn" disabled={grantTeam.isPending || !teamForProject || !projectForTeam}>{t('org.grantAction')}</button>
              </div>
            </form>
          </>
        )}
      </section>
    </main>
  )
}
