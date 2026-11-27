// SPDX-License-Identifier: AGPL-3.0-or-later
import { useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import { useApi } from '../lib/api'
import { shortId, whenIso } from '../lib/format'
import type { CiRun, CiRunCheck, CiCheckOption } from '../lib/types'
import { useProject } from './ProjectLayout'
import { ErrorNote, Loading } from '../components/States'

function runTone(run: CiRun): string {
  if (run.status === 'failed' || (run.status === 'done' && run.passed === false)) return 'bad'
  if (run.status === 'done' && run.passed) return 'ok'
  return 'warn'
}

function runLabel(run: CiRun, t: ReturnType<typeof useT>): string {
  if (run.status === 'done') return run.passed ? t('run.passed') : t('run.failed')
  return run.status === 'running' ? t('run.running') : run.status === 'queued' ? t('run.queued') : t('run.failed')
}

function seconds(value: number | null | undefined): string {
  if (value === null || value === undefined) return '—'
  if (value < 1) return `${Math.round(value * 1000)} ms`
  if (value < 60) return `${value.toFixed(1)} s`
  return `${Math.floor(value / 60)}m ${Math.round(value % 60)}s`
}

function CheckResult({ check, t }: { check: CiRunCheck; t: ReturnType<typeof useT> }) {
  return (
    <section className="stack" style={{ marginTop: '1rem' }}>
      <div className="row" style={{ justifyContent: 'space-between', alignItems: 'baseline' }}>
        <strong>{check.title || check.check}</strong>
        <span className={`tag ${check.failed ? 'bad' : 'ok'}`}>
          {check.failed ? t('run.failed') : t('run.passed')}
        </span>
      </div>
      <p className="small muted">
        {check.total} {t('ci.issues')} · {check.counts?.error ?? 0} {t('ci.errors')} ·{' '}
        {check.counts?.warning ?? 0} {t('ci.warnings')} · {seconds(check.seconds)}
      </p>
      {check.error && <p className="notice bad">{check.error}</p>}
      {!!check.issues?.length && (
        <table className="schedule small">
          <thead>
            <tr>
              <th>{t('ci.severity')}</th>
              <th>{t('ci.message')}</th>
              <th>{t('ci.elements')}</th>
            </tr>
          </thead>
          <tbody>
            {check.issues.slice(0, 200).map((issue, i) => (
              <tr key={i}>
                <td>
                  <span className={`tag ${issue.severity === 'error' ? 'bad' : ''}`}>{issue.severity}</span>
                </td>
                <td>{issue.message}</td>
                <td className="small muted">{issue.guids?.slice(0, 4).join(', ') ?? ''}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  )
}

export function Ci() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const [params, setParams] = useSearchParams()
  const option = params.get('option') ?? ''
  const [open, setOpen] = useState<string | null>(null)

  const runsKey = ctx.api(`/ci/runs?limit=100${option ? `&option=${encodeURIComponent(option)}` : ''}`)
  const runs = useApi<{ runs: CiRun[] }>(runsKey, { refetchInterval: 5000 })
  const required = useApi<{ options: CiCheckOption[]; checks: string[] }>(ctx.api('/ci/checks'), {
    refetchInterval: 10000,
  })
  const list = runs.data?.runs ?? []
  const detail = useApi<{ run: CiRun }>(open ? ctx.api(`/ci/runs/${open}`) : null)
  const fromList = useMemo(() => list.find((r) => r.id === open) ?? null, [list, open])
  const selected = detail.data?.run ?? fromList
  const byCommit = useMemo(() => {
    const map = new Map<string, CiCheckOption>()
    for (const o of required.data?.options ?? []) map.set(o.head, o)
    return map
  }, [required.data])

  const headOf = (commit: string) => byCommit.get(commit)

  return (
    <main className="page">
      <header className="page-head">
        <h1>{t('ci.title')}</h1>
        <div className="row">
          <select
            aria-label={t('ci.option')}
            value={option}
            onChange={(e) => setParams(e.target.value ? { option: e.target.value } : {})}
            style={{ width: 'auto' }}
          >
            <option value="">{t('ci.allOptions')}</option>
            {ctx.overview.options.map((o) => (
              <option key={o.name} value={o.name}>
                {o.label}
              </option>
            ))}
          </select>
        </div>
      </header>

      <h2>{t('ci.required')}</h2>
      {required.isLoading && <Loading />}
      <ErrorNote error={required.error} />
      {required.data?.options.length === 0 && <p className="muted small">{t('ci.noOptions')}</p>}
      {!!required.data?.options.length && (
        <table className="schedule">
          <thead>
            <tr>
              <th>{t('ci.option')}</th>
              <th>{t('ci.requiredChecks')}</th>
              <th>{t('ci.state')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {required.data.options.map((o) => (
              <tr key={o.option}>
                <td>
                  <strong>{o.option}</strong>
                  {o.protected && <span className="tag">{t('ci.protected')}</span>}
                  {o.head && <div className="small muted"><code>{shortId(o.head)}</code></div>}
                </td>
                <td className="small">{o.required.length ? o.required.join(', ') : t('ci.noneRequired')}</td>
                <td>
                  <span className={`tag ${o.state === 'passed' ? 'ok' : o.state === 'failed' ? 'bad' : 'warn'}`}>
                    {o.state === 'passed' ? t('run.passed') : o.state === 'failed' ? t('run.failed')
                      : o.state === 'pending' ? t('run.running') : t('ci.none')}
                  </span>
                </td>
                <td className="num">
                  {o.job && (
                    <Link className="small" to={`${ctx.base}/ci?option=${encodeURIComponent(o.option)}`}>
                      {t('ci.open')}
                    </Link>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h2 style={{ marginTop: '2rem' }}>{t('ci.runs')}</h2>
      {runs.isLoading && <Loading />}
      <ErrorNote error={runs.error} />
      {list.length === 0 && !runs.isLoading && <p className="empty">{t('ci.empty')}</p>}
      {!!list.length && (
        <table className="schedule">
          <thead>
            <tr>
              <th>{t('ci.state')}</th>
              <th>{t('ci.version')}</th>
              <th>{t('ci.option')}</th>
              <th>{t('ci.checks')}</th>
              <th>{t('ci.duration')}</th>
              <th>{t('ci.when')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {list.map((run) => {
              const owner = headOf(run.commit)
              return (
                <tr key={run.id} className="clickable" aria-selected={run.id === open} onClick={() => setOpen(run.id)}>
                  <td>
                    <span className={`tag ${runTone(run)}`}>{runLabel(run, t)}</span>
                    {run.status === 'running' && (
                      <div className="progress" aria-hidden>
                        <span style={{ width: `${Math.max(2, Math.round(run.progress * 100))}%` }} />
                      </div>
                    )}
                  </td>
                  <td>
                    <code>{shortId(run.commit)}</code>
                  </td>
                  <td className="small">{owner?.option ?? ''}</td>
                  <td className="small">{run.checks.join(', ')}</td>
                  <td className="small">{seconds(run.duration)}</td>
                  <td className="small muted">{whenIso(run.created_at, lang)}</td>
                  <td className="num">
                    <a
                      className="small"
                      href={ctx.api(`/ci/runs/${run.id}/report.html`)}
                      target="_blank"
                      rel="noreferrer"
                      onClick={(e) => e.stopPropagation()}
                    >
                      {t('ci.report')}
                    </a>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      )}

      {selected && (
        <section className="form-block" style={{ marginTop: '1.5rem' }}>
          <div className="row" style={{ justifyContent: 'space-between', alignItems: 'baseline' }}>
            <h2>
              {t('ci.detail')} <code>{shortId(selected.commit)}</code>
            </h2>
            <button type="button" className="btn-quiet" onClick={() => setOpen(null)}>
              {t('ci.close')}
            </button>
          </div>
          <p className="small muted">
            {selected.status} · {selected.checks.join(', ')} · {seconds(selected.duration)}
            {selected.error ? ` · ${selected.error}` : ''}
          </p>
          {detail.isLoading && <Loading />}
          <ErrorNote error={detail.error} />
          {selected.status === 'done' && !selected.result && !detail.isLoading && <p className="muted">{t('ci.noResult')}</p>}
          {selected.result?.checks?.map((check, i) => (
            <CheckResult key={i} check={check} t={t} />
          ))}
          <p style={{ marginTop: '1rem' }}>
            <a className="btn" href={ctx.api(`/ci/runs/${selected.id}/report.html`)} target="_blank" rel="noreferrer">
              {t('ci.openReport')}
            </a>
          </p>
        </section>
      )}
    </main>
  )
}
