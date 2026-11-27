// SPDX-License-Identifier: AGPL-3.0-or-later
import { useRef, useState } from 'react'
import { Link, useNavigate, useParams, useSearchParams } from 'react-router-dom'
import { useQueryClient } from '@tanstack/react-query'
import { useLang, useT } from '../i18n'
import { api, ApiError, useApi } from '../lib/api'
import { whenIso } from '../lib/format'
import { useProject } from './ProjectLayout'
import { ErrorNote, Loading } from '../components/States'

interface Issue {
  id: string
  number: number
  title: string
  body: string
  state: 'open' | 'closed'
  priority: string
  author: string
  assignee: string | null
  elements: string[]
  viewpoint: { selected?: string[] } | null
  version: string | null
  created_at: string
}

const PRIORITIES = ['low', 'normal', 'high', 'critical']

export function Issues() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const qc = useQueryClient()
  const [params, setParams] = useSearchParams()
  const state = params.get('state') ?? 'open'
  const list = useApi<Issue[]>(ctx.api(`/issues?state=${state}`))
  const file = useRef<HTMLInputElement>(null)
  const [error, setError] = useState<string | null>(null)
  async function importBcf(f: File) {
    const body = new FormData()
    body.set('file', f)
    const r = await fetch(ctx.api('/issues/import'), { method: 'POST', body, credentials: 'same-origin' })
    if (!r.ok) setError((await r.json()).message)
    qc.invalidateQueries()
  }
  return (
    <main className="page">
      <header className="page-head">
        <h1>{t('issues.title')}</h1>
        <div className="row">
          <a className="btn" href={ctx.api('/issues/export.bcfzip')}>
            {t('issues.export')}
          </a>
          {ctx.can('reviewer') && (
            <>
              <button type="button" onClick={() => file.current?.click()}>
                {t('issues.import')}
              </button>
              <input ref={file} type="file" accept=".bcfzip,.bcf" hidden onChange={(e) => e.target.files?.[0] && importBcf(e.target.files[0])} />
              <Link className="btn btn-primary" to={`${ctx.base}/issues/new`}>
                {t('issues.new')}
              </Link>
            </>
          )}
        </div>
      </header>
      {error && <p className="notice bad">{error}</p>}
      <div className="tabs" role="tablist">
        {(['open', 'closed'] as const).map((s) => (
          <button key={s} role="tab" aria-selected={state === s} onClick={() => setParams({ state: s })}>
            {t(`issues.${s}`)}
          </button>
        ))}
      </div>
      {list.isLoading && <Loading />}
      <ErrorNote error={list.error} />
      {list.data?.length === 0 && <p className="empty">{t('issues.empty')}</p>}
      <table className="schedule">
        <tbody>
          {list.data?.map((i) => (
            <tr key={i.id}>
              <td style={{ width: '4rem' }} className="muted">
                #{i.number}
              </td>
              <td>
                <Link to={`${ctx.base}/issues/${i.number}`}>{i.title}</Link>
                <div className="small muted">
                  {i.author}, {whenIso(i.created_at, lang)}
                </div>
                <div className="small muted show-sm">
                  {i.priority}
                  {i.assignee ? ` · ${i.assignee}` : ''}
                </div>
              </td>
              <td className="small col-prio" style={{ width: '7rem' }}>{i.priority}</td>
              <td className="small col-assignee" style={{ width: '8rem' }}>{i.assignee ?? ''}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </main>
  )
}

export function NewIssue() {
  const t = useT()
  const ctx = useProject()
  const nav = useNavigate()
  const [params] = useSearchParams()
  const element = params.get('element')
  const [title, setTitle] = useState('')
  const [body, setBody] = useState('')
  const [priority, setPriority] = useState('normal')
  const [assignee, setAssignee] = useState('')
  const [error, setError] = useState<string | null>(null)
  return (
    <main className="page page-narrow" style={{ maxWidth: '44rem' }}>
      <h1>{t('issues.new')}</h1>
      <form
        onSubmit={async (e) => {
          e.preventDefault()
          try {
            const i = await api<Issue>(ctx.api('/issues'), {
              method: 'POST',
              json: {
                title,
                body,
                priority,
                assignee: assignee || null,
                elements: element ? [element] : [],
                viewpoint: element ? { selected: [element] } : null,
                version: params.get('version'),
              },
            })
            nav(`${ctx.base}/issues/${i.number}`)
          } catch (err) {
            setError(err instanceof ApiError ? err.message : String(err))
          }
        }}
      >
        {element && (
          <p className="small muted">
            {t('issues.elements')}: <code>{element}</code>
          </p>
        )}
        <div className="field">
          <label htmlFor="title">{t('issues.titleField')}</label>
          <input id="title" value={title} onChange={(e) => setTitle(e.target.value)} required />
        </div>
        <div className="field">
          <label htmlFor="body">{t('issues.body')}</label>
          <textarea id="body" value={body} onChange={(e) => setBody(e.target.value)} />
        </div>
        <div className="row">
          <div className="field">
            <label htmlFor="prio">{t('issues.priority')}</label>
            <select id="prio" value={priority} onChange={(e) => setPriority(e.target.value)}>
              {PRIORITIES.map((p) => (
                <option key={p}>{p}</option>
              ))}
            </select>
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="asg">{t('issues.assignee')}</label>
            <input id="asg" value={assignee} onChange={(e) => setAssignee(e.target.value)} placeholder={t('auth.slug')} />
          </div>
        </div>
        {error && <p className="error">{error}</p>}
        <button className="btn-primary">{t('issues.create')}</button>
      </form>
    </main>
  )
}

export function IssueDetail() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const qc = useQueryClient()
  const { number } = useParams()
  const key = ctx.api(`/issues/${number}`)
  const d = useApi<{ issue: Issue; comments: { id: string; author: string; body: string; created_at: string }[] }>(key)
  const [comment, setComment] = useState('')
  if (d.isLoading) return <div className="page"><Loading /></div>
  if (!d.data) return <div className="page"><ErrorNote error={d.error} /></div>
  const i = d.data.issue
  const refresh = () => qc.invalidateQueries({ queryKey: [key] })
  const selected = i.viewpoint?.selected?.[0] ?? i.elements[0]
  return (
    <main className="page stack" style={{ maxWidth: '52rem' }}>
      <h1>
        {i.title} <span className="muted">#{i.number}</span>
      </h1>
      <p className="row small">
        <span className="tag">{i.state === 'open' ? t('issues.open') : t('issues.closed')}</span>
        <span>
          {t('issues.priority')}: {i.priority}
        </span>
        {i.assignee && (
          <span>
            {t('issues.assignee')}: {i.assignee}
          </span>
        )}
        <span className="muted">
          {i.author}, {whenIso(i.created_at, lang)}
        </span>
      </p>
      {i.body && <p style={{ whiteSpace: 'pre-wrap' }}>{i.body}</p>}
      {selected && (
        <p>
          <Link className="btn" to={`${ctx.base}?select=${encodeURIComponent(selected)}${i.version ? `&rev=${i.version}` : ''}`}>
            {t('issues.viewpoint')}
          </Link>{' '}
          <span className="small muted">
            {t('issues.elements')}: {i.elements.join(', ')}
          </span>
        </p>
      )}
      <ul className="list">
        {d.data.comments.map((c) => (
          <li key={c.id}>
            <strong>{c.author}</strong> <span className="small muted">{whenIso(c.created_at, lang)}</span>
            <p style={{ whiteSpace: 'pre-wrap', marginTop: '0.25rem' }}>{c.body}</p>
          </li>
        ))}
      </ul>
      <form
        className="stack"
        onSubmit={async (e) => {
          e.preventDefault()
          await api(ctx.api(`/issues/${i.number}/comments`), { method: 'POST', json: { body: comment } })
          setComment('')
          refresh()
        }}
      >
        <textarea aria-label={t('issues.comment')} value={comment} onChange={(e) => setComment(e.target.value)} />
        <div className="row">
          <button disabled={!comment.trim()}>{t('issues.comment')}</button>
          {ctx.can('reviewer') && (
            <button
              type="button"
              className="btn-quiet"
              onClick={async () => {
                await api(ctx.api(`/issues/${i.number}`), { method: 'PATCH', json: { state: i.state === 'open' ? 'closed' : 'open' } })
                refresh()
              }}
            >
              {i.state === 'open' ? t('issues.close') : t('issues.reopen')}
            </button>
          )}
        </div>
      </form>
    </main>
  )
}
