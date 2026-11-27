// SPDX-License-Identifier: AGPL-3.0-or-later
import { Link, useSearchParams } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import { useApi } from '../lib/api'
import { optionLabel, shortId, when } from '../lib/format'
import type { Version } from '../lib/types'
import { useProject } from './ProjectLayout'
import { ErrorNote, Loading } from '../components/States'

export function SummaryLine({ v }: { v: Version }) {
  const t = useT()
  const s = v.summary
  if (!s) return <span className="muted small">{t('versions.pending')}</span>
  if (s.first) return <span className="small">{t('versions.first', { n: s.added })}</span>
  return (
    <span className="small row" style={{ gap: '0.9rem' }}>
      {s.added > 0 && <span className="change added">{s.added}</span>}
      {s.modified > 0 && <span className="change modified">{s.modified}</span>}
      {s.removed > 0 && <span className="change removed">{s.removed}</span>}
      <span className="muted">{t('versions.summary', { added: s.added, modified: s.modified, removed: s.removed })}</span>
    </span>
  )
}

export function Versions() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const [params, setParams] = useSearchParams()
  const option = params.get('option') ?? ctx.overview.project.default_option
  const before = params.get('before')
  const list = useApi<Version[]>(
    ctx.api(`/versions?option=${encodeURIComponent(option)}${before ? `&before=${before}` : ''}`),
    { refetchInterval: 5000 },
  )
  const last = list.data?.[list.data.length - 1]
  return (
    <main className="page">
      <header className="page-head">
        <h1>{t('versions.title', { option: optionLabel(option) })}</h1>
        <select aria-label={t('title.option')} value={option} onChange={(e) => setParams({ option: e.target.value })} style={{ width: 'auto' }}>
          {ctx.overview.options.map((o) => (
            <option key={o.name} value={o.name}>
              {o.label}
            </option>
          ))}
        </select>
      </header>
      {list.isLoading && <Loading />}
      <ErrorNote error={list.error} />
      {list.data?.length === 0 && <p className="empty">{t('versions.empty')}</p>}
      <ol className="list">
        {list.data?.map((v) => (
          <li key={v.id}>
            <div className="row" style={{ justifyContent: 'space-between', alignItems: 'baseline' }}>
              <strong>{v.message}</strong>
              <span className="small muted">
                {v.author}, {when(v.time, lang)} <code>{shortId(v.id)}</code>
              </span>
            </div>
            <div className="row" style={{ justifyContent: 'space-between', marginTop: '0.25rem' }}>
              <SummaryLine v={v} />
              {v.parents.length > 0 && (
                <Link className="small" to={`${ctx.base}/compare?from=${v.parents[0]}&to=${v.id}`}>
                  {t('versions.compareParent')}
                </Link>
              )}
            </div>
          </li>
        ))}
      </ol>
      {list.data && list.data.length >= 50 && last && (
        <p style={{ marginTop: '1rem' }}>
          <button type="button" onClick={() => setParams({ option, before: last.id })}>
            {t('versions.more')}
          </button>
        </p>
      )}
    </main>
  )
}
