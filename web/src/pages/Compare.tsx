// SPDX-License-Identifier: AGPL-3.0-or-later
import { useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { useT } from '../i18n'
import { useJob } from '../lib/api'
import { ifcClass, shortId } from '../lib/format'
import type { CompareResult, ElementChange, SettingsDiff } from '../lib/types'
import type { ChangeKind } from '../viewer/colors'
import { Viewer } from '../viewer/Viewer'
import { useProject } from './ProjectLayout'
import { ErrorNote, JobWait } from '../components/States'

function show(v: unknown): string {
  if (v === null || v === undefined || v === '') return '—'
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}

function FieldTable({ change }: { change: ElementChange }) {
  const t = useT()
  const rows = [
    ...change.attributes.map((a) => ({ field: a.name === 'ObjectPlacement' ? t('compare.placement') : a.name === 'Representation' ? t('compare.geometry') : a.name, old: a.old, new: a.new })),
    ...change.properties.map((p) => ({ field: p.pset ? `${p.pset}.${p.name}` : p.name, old: p.old, new: p.new })),
  ]
  if (!rows.length) return null
  return (
    <table className="schedule small">
      <thead>
        <tr>
          <th>{t('compare.field')}</th>
          <th>{t('compare.before')}</th>
          <th>{t('compare.after')}</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr key={i}>
            <td>{r.field}</td>
            <td className="diffval-old">{show(r.old)}</td>
            <td className="diffval-new">{show(r.new)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

export function SettingsChanges({ from, to, protectedKeys = [] }: { from: string; to: string; protectedKeys?: string[] }) {
  const t = useT()
  const { api } = useProject()
  const diff = useJob<SettingsDiff>(api(`/settings-diff?from=${from}&to=${to}`))
  const d = diff.data
  if (!d || d.pending) return null
  const changed = d.data.changed
  return (
    <section>
      <h3>{t('compare.settings')}</h3>
      {changed.length === 0 ? (
        <p className="small muted">{t('compare.settingsNone')}</p>
      ) : (
        <table className="schedule small">
          <tbody>
            {changed.map((c) => (
              <tr key={c.key}>
                <td>
                  {c.key} {protectedKeys.includes(c.key) && <span className="tag">{t('compare.protected')}</span>}
                </td>
                <td className="diffval-old">{show(c.old)}</td>
                <td className="diffval-new">{show(c.new)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  )
}

/** The compare view: coloured model, change list by level and class, and field-by-field detail. */
export function CompareView({ from, to, protectedKeys }: { from: string; to: string; protectedKeys?: string[] }) {
  const t = useT()
  const { api } = useProject()
  const result = useJob<CompareResult>(api(`/compare?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`))
  const [selected, setSelected] = useState<string | null>(null)
  const r = result.data
  const data = r && !r.pending ? r.data : null
  const highlight = useMemo(() => {
    const h: Record<string, ChangeKind> = {}
    for (const d of data?.details ?? []) h[d.guid] = d.kind
    return h
  }, [data])
  const byLevel = useMemo(() => {
    const m = new Map<string, ElementChange[]>()
    for (const d of data?.details ?? []) {
      const k = d.level || ''
      m.set(k, [...(m.get(k) ?? []), d])
    }
    return [...m.entries()].sort(([a], [b]) => (a === '' ? 1 : b === '' ? -1 : a.localeCompare(b)))
  }, [data])
  const current = data?.details.find((d) => d.guid === selected)

  if (result.error) return <div className="page"><ErrorNote error={result.error} /></div>
  if (!data) return <div className="page"><JobWait progress={result.data?.pending ? result.data.progress : 0} label={t('compare.working')} /></div>
  const counts = { added: 0, modified: 0, removed: 0 }
  for (const d of data.details) counts[d.kind]++

  return (
    <div className="split">
      <Viewer model={api(`/versions/${data.to}/model.ifc`)} ghostModel={api(`/versions/${data.from}/model.ifc`)} highlight={highlight} selected={selected} onSelect={(g) => setSelected(g)} />
      <aside className="side">
        <p className="row small">
          <span className="change added">{counts.added} {t('compare.added')}</span>
          <span className="change modified">{counts.modified} {t('compare.modified')}</span>
          <span className="change removed">{counts.removed} {t('compare.removed')}</span>
        </p>
        <p className="small muted">
          <code>{shortId(data.from)}</code> → <code>{shortId(data.to)}</code>
        </p>
        {data.details.length === 0 && <p>{t('compare.none')}</p>}
        {current && (
          <section style={{ marginBottom: '1rem' }}>
            <h3>
              <span className={`change ${current.kind}`}>{current.name || ifcClass(current.type)}</span>
            </h3>
            <p className="small muted">
              {ifcClass(current.type)} · {current.guid}
            </p>
            <FieldTable change={current} />
          </section>
        )}
        <h3>{t('compare.byLevel')}</h3>
        {byLevel.map(([level, items]) => (
          <section key={level}>
            <h4 style={{ margin: '0.75rem 0 0.25rem' }}>{level || t('compare.noLevel')}</h4>
            <table className="schedule small">
              <tbody>
                {items.map((d) => (
                  <tr key={d.guid} className="clickable" onClick={() => setSelected(d.guid)} aria-selected={d.guid === selected}>
                    <td>
                      <span className={`change ${d.kind}`}>{d.name || '—'}</span>
                    </td>
                    <td className="muted">{ifcClass(d.type)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        ))}
        {data.other > 0 && <p className="small muted">{t('compare.other', { n: data.other })}</p>}
        <SettingsChanges from={data.from} to={data.to} protectedKeys={protectedKeys} />
      </aside>
    </div>
  )
}

export function Compare() {
  const t = useT()
  const ctx = useProject()
  const [params, setParams] = useSearchParams()
  const opts = ctx.overview.options
  const [from, setFrom] = useState(params.get('from') ?? (opts[0] ? `${opts[0].name}~1` : ''))
  const [to, setTo] = useState(params.get('to') ?? opts[0]?.name ?? '')
  const active = params.get('from') && params.get('to')
  return (
    <div style={{ display: 'grid', gridTemplateRows: 'auto 1fr', height: '100%' }}>
      <div className="compare-bar">
        <div className="shell">
          <h1>{t('compare.title')}</h1>
          <form
            className="row"
            onSubmit={(e) => {
              e.preventDefault()
              setParams({ from, to })
            }}
          >
            <label className="row" style={{ fontWeight: 400 }}>
              {t('compare.from')}
              <input list="revs" value={from} onChange={(e) => setFrom(e.target.value)} style={{ width: '14rem' }} />
            </label>
            <label className="row" style={{ fontWeight: 400 }}>
              {t('compare.to')}
              <input list="revs" value={to} onChange={(e) => setTo(e.target.value)} style={{ width: '14rem' }} />
            </label>
            <datalist id="revs">
              {opts.map((o) => (
                <option key={o.name} value={o.name}>
                  {o.label}
                </option>
              ))}
              {ctx.overview.milestones?.map((m) => <option key={m.name} value={m.name} />)}
            </datalist>
            <button className="btn-primary">{t('compare.run')}</button>
          </form>
        </div>
      </div>
      {active ? <CompareView key={`${params.get('from')}..${params.get('to')}`} from={params.get('from')!} to={params.get('to')!} /> : null}
    </div>
  )
}
