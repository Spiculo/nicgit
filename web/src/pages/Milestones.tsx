// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import type { Key } from '../i18n/en'
import { api, ApiError, useAction, useApi } from '../lib/api'
import { shortId, when, whenIso } from '../lib/format'
import { useProject } from './ProjectLayout'
import { ErrorNote, Loading } from '../components/States'

const PHASES = ['concept', 'schematic', 'design_development', 'permit', 'tender', 'construction', 'as_built'] as const

interface Tag {
  name: string
  target: string
  message: string
  tagger: string
  time: number
}

interface Milestone {
  id: string
  phase: (typeof PHASES)[number]
  tag: string
  commit: string
  title: string
  notes: string
  created_by: string
  signed_off_by: string | null
  signed_off_at: string | null
  created_at: string
}

export function Milestones() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const key = ctx.api('/milestones')
  const list = useApi<Milestone[]>(key)
  const tags = useApi<Tag[]>(ctx.api('/tags'))
  const [phase, setPhase] = useState<(typeof PHASES)[number]>('concept')
  const [tag, setTag] = useState('')
  const [rev, setRev] = useState(ctx.overview.project.default_option)
  const [title, setTitle] = useState('')
  const [notes, setNotes] = useState('')
  const [error, setError] = useState<string | null>(null)
  const create = useAction(() => api(key, { method: 'POST', json: { phase, tag, rev, title, notes } }), [[key], [ctx.api('/overview')]])
  const sign = useAction((id: string) => api(ctx.api(`/milestones/${id}/signoff`), { method: 'POST' }), [[key]])
  const ordered = [...(list.data ?? [])].sort((a, b) => PHASES.indexOf(a.phase) - PHASES.indexOf(b.phase))
  return (
    <main className="page stack">
      <h1>{t('milestones.title')}</h1>
      {list.isLoading && <Loading />}
      <ErrorNote error={list.error} />
      {list.data?.length === 0 && <p className="empty">{t('milestones.empty')}</p>}
      {!!ordered.length && (
        <table className="schedule">
          <thead>
            <tr>
              <th>{t('milestones.phase')}</th>
              <th>{t('milestones.tag')}</th>
              <th>{t('milestones.version')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {ordered.map((m) => (
              <tr key={m.id}>
                <td>{t(`phase.${m.phase}` as Key)}</td>
                <td>
                  <strong>{m.title}</strong>
                  <div className="small muted">
                    {m.tag}. {m.created_by}, {whenIso(m.created_at, lang)}
                  </div>
                  {m.notes && <div className="small">{m.notes}</div>}
                </td>
                <td>
                  <code>{shortId(m.commit)}</code>
                  <div className="small">
                    <a href={ctx.api(`/versions/${m.commit}/model.ifc`)} download>
                      IFC
                    </a>{' '}
                    <a href={ctx.api(`/archive?rev=${encodeURIComponent(m.tag)}`)} download>
                      {t('milestones.source')}
                    </a>{' '}
                    <Link to={`${ctx.base}?rev=${m.commit}`}>{t('rail.model')}</Link>
                  </div>
                </td>
                <td className="num">
                  {m.signed_off_by ? (
                    <span className="small">{t('milestones.signedOff', { who: m.signed_off_by })}</span>
                  ) : (
                    ctx.can('lead') && (
                      <button type="button" onClick={() => sign.mutate(m.id)}>
                        {t('milestones.signOff')}
                      </button>
                    )
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h2>{t('milestones.tags')}</h2>
      {tags.isLoading && <Loading />}
      <ErrorNote error={tags.error} />
      {tags.data?.length === 0 && <p className="empty">{t('milestones.tagsEmpty')}</p>}
      {!!tags.data?.length && (
        <table className="schedule">
          <thead>
            <tr>
              <th>{t('milestones.tag')}</th>
              <th>{t('milestones.version')}</th>
              <th>{t('branches.author')}</th>
              <th>{t('branches.updated')}</th>
            </tr>
          </thead>
          <tbody>
            {tags.data.map((tag) => (
              <tr key={tag.name}>
                <td>
                  <strong>{tag.name}</strong>
                  {tag.message && <div className="small muted">{tag.message.split('\n')[0]}</div>}
                </td>
                <td>
                  <code>{shortId(tag.target)}</code>
                  <div className="small">
                    <a href={ctx.api(`/versions/${tag.name}/model.ifc`)} download>IFC</a>{' '}
                    <a href={ctx.api(`/archive?rev=${encodeURIComponent(tag.name)}`)} download>{t('milestones.source')}</a>{' '}
                    <Link to={`${ctx.base}?rev=${tag.target}`}>{t('rail.model')}</Link>
                  </div>
                </td>
                <td className="small">{tag.tagger}</td>
                <td className="small">{when(tag.time, lang)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {ctx.can('lead') && !ctx.overview.empty && (
        <form
          className="stack"
          style={{ maxWidth: '40rem', marginTop: '2rem' }}
          onSubmit={async (e) => {
            e.preventDefault()
            setError(null)
            try {
              await create.mutateAsync(undefined)
              setTag('')
              setTitle('')
              setNotes('')
            } catch (err) {
              setError(err instanceof ApiError ? err.message : String(err))
            }
          }}
        >
          <h2>{t('milestones.new')}</h2>
          <div className="row">
            <div className="field" style={{ flex: 1 }}>
              <label htmlFor="ph">{t('milestones.phase')}</label>
              <select id="ph" value={phase} onChange={(e) => setPhase(e.target.value as (typeof PHASES)[number])}>
                {PHASES.map((p) => (
                  <option key={p} value={p}>
                    {t(`phase.${p}` as Key)}
                  </option>
                ))}
              </select>
            </div>
            <div className="field" style={{ flex: 1 }}>
              <label htmlFor="rv">{t('milestones.version')}</label>
              <select id="rv" value={rev} onChange={(e) => setRev(e.target.value)}>
                {ctx.overview.options.map((o) => (
                  <option key={o.name} value={o.name}>
                    {o.label}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <div className="field">
            <label htmlFor="tt">{t('proposals.titleField')}</label>
            <input id="tt" value={title} onChange={(e) => setTitle(e.target.value)} required />
          </div>
          <div className="field">
            <label htmlFor="tg">{t('milestones.tag')}</label>
            <input id="tg" value={tag} onChange={(e) => setTag(e.target.value.replace(/\s+/g, '-'))} required placeholder="permit-set-1" />
          </div>
          <div className="field">
            <label htmlFor="nt">{t('issues.body')}</label>
            <textarea id="nt" value={notes} onChange={(e) => setNotes(e.target.value)} />
          </div>
          {error && <p className="error">{error}</p>}
          <div>
            <button className="btn-primary">{t('milestones.create')}</button>
          </div>
        </form>
      )}
    </main>
  )
}
