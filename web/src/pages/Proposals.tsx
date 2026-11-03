// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Link, useNavigate, useParams, useSearchParams } from 'react-router-dom'
import { useQueryClient } from '@tanstack/react-query'
import { useLang, useT } from '../i18n'
import { api, ApiError, useApi, useJob } from '../lib/api'
import { optionLabel, shortId, whenIso, ifcClass } from '../lib/format'
import { useProject } from './ProjectLayout'
import { CompareView } from './Compare'
import { ErrorNote, JobWait, Loading } from '../components/States'

interface Proposal {
  id: string
  number: number
  title: string
  body: string
  source: string
  target: string
  author: string
  state: 'open' | 'combined' | 'closed'
  source_head: string
  target_head: string
  merge_commit: string | null
  created_at: string
}

interface Missing {
  kind: 'approvals' | 'role' | 'changes_requested' | 'settings_approval'
  needed?: number
  have?: number
  [k: string]: unknown
}

interface Detail {
  proposal: Proposal
  reviews: { user: string; verdict: string; body: string; dismissed: boolean; created_at: string }[]
  comments: { id: string; author: string; body: string; pin: { elements?: string[] } | null; created_at: string }[]
  rule: { allowed_modes: string[]; required_approvals: number; protected_settings: string[] }
  protected: boolean
  approvals: { ok: boolean; approvals: number; missing: Missing[] }
  settings_changed: string[]
  can_combine: boolean
  can_review: boolean
  ai_summary: { text: string } | null
}

interface Conflict {
  id: string
  kind: string
  key: string
  ifc_type: string
  name: string
  attribute: string | null
  property: string | null
  base: unknown
  ours: unknown
  theirs: unknown
  message: string
}

interface Preview {
  status: 'up_to_date' | 'fast_forward' | 'merge'
  conflicts: Conflict[]
  settings_conflicts: { key: string; base: unknown; ours: unknown; theirs: unknown }[]
  warnings: unknown[]
}

function StateTag({ state }: { state: Proposal['state'] }) {
  const t = useT()
  const label = state === 'open' ? t('proposals.open') : state === 'combined' ? t('proposals.combined') : t('proposals.closed')
  return <span className="tag" style={state === 'combined' ? { borderColor: 'var(--added)', color: 'var(--added)' } : undefined}>{label}</span>
}

export function Proposals() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const [params, setParams] = useSearchParams()
  const state = params.get('state') ?? 'open'
  const list = useApi<Proposal[]>(ctx.api(`/proposals?state=${state}`))
  return (
    <main className="page">
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <h1>{t('proposals.title')}</h1>
        {ctx.can('member') && (
          <Link className="btn btn-primary" to={`${ctx.base}/proposals/new`}>
            {t('proposals.new')}
          </Link>
        )}
      </div>
      <div className="tabs" role="tablist">
        {(['open', 'combined', 'closed'] as const).map((s) => (
          <button key={s} role="tab" aria-selected={state === s} onClick={() => setParams({ state: s })}>
            {t(`proposals.${s}`)}
          </button>
        ))}
      </div>
      {list.isLoading && <Loading />}
      <ErrorNote error={list.error} />
      {list.data?.length === 0 && <p className="empty">{t('proposals.empty')}</p>}
      <ul className="list">
        {list.data?.map((p) => (
          <li key={p.id}>
            <Link to={`${ctx.base}/proposals/${p.number}`}>
              <strong>
                #{p.number} {p.title}
              </strong>
            </Link>
            <div className="small muted">
              {t('proposals.into', { source: optionLabel(p.source), target: optionLabel(p.target) })}. {p.author}, {whenIso(p.created_at, lang)}
            </div>
          </li>
        ))}
      </ul>
    </main>
  )
}

export function NewProposal() {
  const t = useT()
  const ctx = useProject()
  const nav = useNavigate()
  const opts = ctx.overview.options
  const [source, setSource] = useState(opts.find((o) => !o.is_default)?.name ?? '')
  const [target, setTarget] = useState(opts.find((o) => o.is_default)?.name ?? '')
  const [title, setTitle] = useState('')
  const [body, setBody] = useState('')
  const [error, setError] = useState<string | null>(null)
  return (
    <main className="page page-narrow" style={{ maxWidth: '44rem' }}>
      <h1>{t('proposals.new')}</h1>
      <form
        onSubmit={async (e) => {
          e.preventDefault()
          try {
            const p = await api<Proposal>(ctx.api('/proposals'), { method: 'POST', json: { source, target, title, body } })
            nav(`${ctx.base}/proposals/${p.number}`)
          } catch (err) {
            setError(err instanceof ApiError ? err.message : String(err))
          }
        }}
      >
        <div className="row">
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="src">{t('proposals.source')}</label>
            <select id="src" value={source} onChange={(e) => setSource(e.target.value)}>
              {opts.map((o) => (
                <option key={o.name} value={o.name}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="tgt">{t('proposals.target')}</label>
            <select id="tgt" value={target} onChange={(e) => setTarget(e.target.value)}>
              {opts.map((o) => (
                <option key={o.name} value={o.name}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="field">
          <label htmlFor="title">{t('proposals.titleField')}</label>
          <input id="title" value={title} onChange={(e) => setTitle(e.target.value)} required />
        </div>
        <div className="field">
          <label htmlFor="body">{t('proposals.body')}</label>
          <textarea id="body" value={body} onChange={(e) => setBody(e.target.value)} />
        </div>
        {error && <p className="error">{error}</p>}
        <button className="btn-primary" disabled={!source || !target || source === target}>
          {t('proposals.create')}
        </button>
      </form>
    </main>
  )
}

function show(v: unknown): string {
  if (v === null || v === undefined || v === '') return '—'
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}

function MissingLine({ m }: { m: Missing }) {
  const t = useT()
  if (m.kind === 'approvals') return <li>{t('proposals.needs', { n: (m.needed ?? 0) - (m.have ?? 0) })}</li>
  if (m.kind === 'role') return <li>{t('proposals.needsRole', { role: t(`role.${String(m.role ?? 'lead')}` as 'role.lead') })}</li>
  if (m.kind === 'settings_approval') return <li>{t('compare.protected')}: {t('proposals.needsRole', { role: t('role.lead') })}</li>
  return <li>{t('proposals.requestChanges')}</li>
}

function Combine({ d, onDone }: { d: Detail; onDone: () => void }) {
  const t = useT()
  const ctx = useProject()
  const p = d.proposal
  const preview = useJob<Preview>(ctx.api(`/proposals/${p.number}/preview`))
  const allowed = d.rule.allowed_modes
  const [mode, setMode] = useState<string>(allowed[0] ?? 'auto')
  const [picks, setPicks] = useState<Record<string, 'ours' | 'theirs'>>({})
  const [settingsPicks, setSettingsPicks] = useState<Record<string, 'ours' | 'theirs'>>({})
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const pv = preview.data
  if (!pv || pv.pending) return <JobWait progress={pv?.pending ? pv.progress : 0} />
  const data = pv.data
  const source = optionLabel(p.source)
  const target = optionLabel(p.target)
  const conflicts = data.conflicts
  const settingsConflicts = data.settings_conflicts
  const total = conflicts.length + settingsConflicts.length
  const modeLabel = (m: string) =>
    m === 'auto' ? t('proposals.modeAuto') : m === 'whole:ours' ? t('proposals.modeOurs', { target }) : m === 'whole:theirs' ? t('proposals.modeTheirs', { source }) : t('proposals.modeGranular')
  const ready =
    d.approvals.ok &&
    d.can_combine &&
    (total === 0 || (mode !== 'auto' && (mode !== 'granular' || (conflicts.every((c) => picks[c.id]) && settingsConflicts.every((s) => settingsPicks[s.key])))))
  async function go() {
    setBusy(true)
    setError(null)
    try {
      await api(ctx.api(`/proposals/${p.number}/combine`), { method: 'POST', json: { mode: total === 0 ? 'auto' : mode, picks, settings_picks: settingsPicks } })
      onDone()
    } catch (e) {
      setError(e instanceof ApiError ? e.message : String(e))
    }
    setBusy(false)
  }
  const sidePick = (value: string | undefined, set: (v: 'ours' | 'theirs') => void, name: string) => (
    <div className="row small">
      <label style={{ fontWeight: 400, display: 'flex', gap: '0.3rem' }}>
        <input type="radio" name={name} style={{ width: 'auto' }} checked={value === 'ours'} onChange={() => set('ours')} />
        {t('proposals.keepTarget', { target })}
      </label>
      <label style={{ fontWeight: 400, display: 'flex', gap: '0.3rem' }}>
        <input type="radio" name={name} style={{ width: 'auto' }} checked={value === 'theirs'} onChange={() => set('theirs')} />
        {t('proposals.takeSource', { source })}
      </label>
    </div>
  )
  return (
    <section className="stack">
      {data.status === 'up_to_date' && <p className="notice">{t('proposals.upToDate', { target, source })}</p>}
      {data.status === 'fast_forward' && <p className="notice">{t('proposals.fastForward', { target })}</p>}
      {data.status === 'merge' && total === 0 && <p className="notice">{t('proposals.noConflicts')}</p>}
      {total > 0 && (
        <>
          <p className="notice warn">{t('proposals.conflictsCount', { n: total })}</p>
          <fieldset style={{ border: 0, padding: 0 }}>
            <legend style={{ fontWeight: 560, marginBottom: '0.4rem' }}>{t('proposals.mode')}</legend>
            {(['auto', 'whole:ours', 'whole:theirs', 'granular'] as const)
              .filter((m) => allowed.includes(m))
              .map((m) => (
                <label key={m} style={{ fontWeight: 400, display: 'flex', gap: '0.4rem' }}>
                  <input type="radio" name="mode" style={{ width: 'auto' }} checked={mode === m} onChange={() => setMode(m)} />
                  {modeLabel(m)}
                </label>
              ))}
          </fieldset>
          <table className="schedule small">
            <thead>
              <tr>
                <th>{t('viewer.properties')}</th>
                <th>{t('proposals.base')}</th>
                <th>{target}</th>
                <th>{source}</th>
              </tr>
            </thead>
            <tbody>
              {conflicts.map((c) => (
                <tr key={c.id}>
                  <td>
                    <strong>{c.name || ifcClass(c.ifc_type)}</strong>
                    <div className="muted">
                      {ifcClass(c.ifc_type)}
                      {c.attribute ? `, ${c.attribute}` : ''}
                      {c.property ? `, ${c.property}` : ''}
                    </div>
                    {c.message && <div className="muted">{c.message}</div>}
                    {mode === 'granular' && sidePick(picks[c.id], (v) => setPicks({ ...picks, [c.id]: v }), c.id)}
                  </td>
                  <td>{show(c.base)}</td>
                  <td>{show(c.ours)}</td>
                  <td>{show(c.theirs)}</td>
                </tr>
              ))}
              {settingsConflicts.map((s) => (
                <tr key={s.key}>
                  <td>
                    <strong>{t('proposals.settingsConflict', { key: s.key })}</strong>
                    {d.rule.protected_settings.includes(s.key) && <span className="tag">{t('compare.protected')}</span>}
                    {mode === 'granular' && sidePick(settingsPicks[s.key], (v) => setSettingsPicks({ ...settingsPicks, [s.key]: v }), `s-${s.key}`)}
                  </td>
                  <td>{show(s.base)}</td>
                  <td>{show(s.ours)}</td>
                  <td>{show(s.theirs)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      {error && <p className="notice bad">{error}</p>}
      {d.can_combine && (
        <div>
          <button className="btn-primary" disabled={!ready || busy || data.status === 'up_to_date'} onClick={go}>
            {busy ? t('proposals.combining') : t('proposals.combine')}
          </button>
        </div>
      )}
    </section>
  )
}

export function ProposalDetail() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const qc = useQueryClient()
  const { number } = useParams()
  const key = ctx.api(`/proposals/${number}`)
  const detail = useApi<Detail>(key)
  const [tab, setTab] = useState<'overview' | 'changes' | 'conflicts'>('overview')
  const [reviewBody, setReviewBody] = useState('')
  if (detail.isLoading) return <div className="page"><Loading /></div>
  if (!detail.data) return <div className="page"><ErrorNote error={detail.error} /></div>
  const d = detail.data
  const p = d.proposal
  const refresh = () => {
    qc.invalidateQueries({ queryKey: [key] })
    qc.invalidateQueries({ queryKey: [ctx.api('/overview')] })
  }
  async function review(verdict: string) {
    await api(ctx.api(`/proposals/${p.number}/reviews`), { method: 'POST', json: { verdict, body: reviewBody } })
    setReviewBody('')
    refresh()
  }
  return (
    <div style={{ display: 'grid', gridTemplateRows: 'auto 1fr', height: '100%' }}>
      <header style={{ padding: '1.25rem 1.5rem 0', background: 'var(--sheet)', borderBottom: '1px solid var(--grid)' }}>
        <h1 style={{ marginBottom: '0.4rem' }}>
          {p.title} <span className="muted">#{p.number}</span>
        </h1>
        <p className="row small">
          <StateTag state={p.state} />
          <span>{t('proposals.into', { source: optionLabel(p.source), target: optionLabel(p.target) })}</span>
          <span className="muted">
            {p.author}, {whenIso(p.created_at, lang)}
          </span>
        </p>
        <div className="tabs" role="tablist" style={{ marginBottom: 0, borderBottom: 0 }}>
          {(['overview', 'changes', 'conflicts'] as const).map((k) => (
            <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)}>
              {k === 'overview' ? t('proposals.tabOverview') : k === 'changes' ? t('proposals.tabChanges') : t('proposals.tabConflicts')}
            </button>
          ))}
        </div>
      </header>
      {tab === 'changes' && <CompareView from={p.state === 'combined' ? `${p.merge_commit}^1` : p.target_head} to={p.source_head} protectedKeys={d.rule.protected_settings} />}
      {tab !== 'changes' && (
        <main className="page stack" style={{ maxWidth: '60rem' }}>
          {tab === 'overview' && (
            <>
              {p.body && <p style={{ whiteSpace: 'pre-wrap' }}>{p.body}</p>}
              {d.ai_summary && (
                <section className="notice">
                  <strong>{t('proposals.aiSummary')}</strong>
                  <p style={{ whiteSpace: 'pre-wrap', marginTop: '0.4rem' }}>{d.ai_summary.text}</p>
                </section>
              )}
              <section>
                <h2>{t('proposals.approvals')}</h2>
                {p.state === 'combined' ? (
                  <p className="notice">
                    {t('proposals.combinedInto', { target: optionLabel(p.target) })} <code>{shortId(p.merge_commit ?? '')}</code>
                  </p>
                ) : d.approvals.ok ? (
                  <p className="notice">{t('proposals.ready')}</p>
                ) : (
                  <ul className="notice warn" style={{ margin: 0, paddingLeft: '1.5rem' }}>
                    {d.approvals.missing.map((m, i) => (
                      <MissingLine key={i} m={m} />
                    ))}
                  </ul>
                )}
                <ul className="list" style={{ marginTop: '1rem' }}>
                  {d.reviews.map((r, i) => (
                    <li key={i} className={r.dismissed ? 'muted' : ''}>
                      <strong>{r.user}</strong>{' '}
                      {r.verdict === 'approve' ? t('proposals.approve') : r.verdict === 'changes' ? t('proposals.requestChanges') : t('proposals.comment')}
                      {r.dismissed && <span className="small"> ({t('proposals.dismissed')})</span>}
                      {r.body && <p style={{ marginTop: '0.25rem' }}>{r.body}</p>}
                    </li>
                  ))}
                  {d.comments.map((c) => (
                    <li key={c.id}>
                      <strong>{c.author}</strong> <span className="small muted">{whenIso(c.created_at, lang)}</span>
                      <p style={{ marginTop: '0.25rem', whiteSpace: 'pre-wrap' }}>{c.body}</p>
                      {c.pin?.elements?.length ? <div className="small muted">{c.pin.elements.join(', ')}</div> : null}
                    </li>
                  ))}
                </ul>
              </section>
              {p.state === 'open' && (
                <section className="stack">
                  <label htmlFor="rv">{t('proposals.reviewBody')}</label>
                  <textarea id="rv" value={reviewBody} onChange={(e) => setReviewBody(e.target.value)} />
                  <div className="row">
                    {d.can_review && (
                      <>
                        <button className="btn-primary" onClick={() => review('approve')}>
                          {t('proposals.approve')}
                        </button>
                        <button onClick={() => review('changes')}>{t('proposals.requestChanges')}</button>
                      </>
                    )}
                    <button
                      onClick={async () => {
                        await api(ctx.api(`/proposals/${p.number}/comments`), { method: 'POST', json: { body: reviewBody } })
                        setReviewBody('')
                        refresh()
                      }}
                      disabled={!reviewBody.trim()}
                    >
                      {t('proposals.comment')}
                    </button>
                    <span style={{ flex: 1 }} />
                    <button
                      className="btn-quiet"
                      onClick={async () => {
                        await api(ctx.api(`/proposals/${p.number}/close`), { method: 'POST' })
                        refresh()
                      }}
                    >
                      {t('proposals.close')}
                    </button>
                  </div>
                </section>
              )}
            </>
          )}
          {tab === 'conflicts' && p.state === 'open' && <Combine d={d} onDone={refresh} />}
          {tab === 'conflicts' && p.state !== 'open' && <p className="muted">{t('proposals.combinedInto', { target: optionLabel(p.target) })}</p>}
        </main>
      )}
    </div>
  )
}
