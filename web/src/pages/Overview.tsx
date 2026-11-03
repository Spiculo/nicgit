// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import { useApi, useJob, useMe } from '../lib/api'
import { optionLabel, optionPath, shortId, when, ifcClass } from '../lib/format'
import type { Version } from '../lib/types'
import { Viewer } from '../viewer/Viewer'
import type { PropertyGroup } from '../viewer/ifc'
import { useProject } from './ProjectLayout'
import { JobWait } from '../components/States'

export function TitleBlock({ title, option, version }: { title: string; option: string; version?: Version | null }) {
  const t = useT()
  const lang = useLang()
  return (
    <div className="titleblock" aria-label={t('title.version')}>
      <div className="tb-title">{title}</div>
      <div>
        <span className="k">{t('title.option')}</span>
        <span className="v">{optionLabel(option)}</span>
      </div>
      <div>
        <span className="k">{t('title.version')}</span>
        <span className="v">{version ? shortId(version.id) : ''}</span>
      </div>
      <div>
        <span className="k">{t('title.by')}</span>
        <span className="v">{version?.author ?? ''}</span>
      </div>
      <div className="tb-last" style={{ gridColumn: '1 / 3' }}>
        <span className="k">{t('overview.latest')}</span>
        <span className="v">{version?.message ?? ''}</span>
      </div>
      <div className="tb-last">
        <span className="k">{t('title.date')}</span>
        <span className="v">{when(version?.time, lang)}</span>
      </div>
    </div>
  )
}

function EmptyProject() {
  const t = useT()
  const { overview } = useProject()
  const me = useMe()
  return (
    <div className="page">
      <div className="empty stack">
        <h2>{t('overview.empty')}</h2>
        <p>{t('overview.emptyHint')}</p>
        <p>
          <code>{overview.clone_url}</code>
        </p>
        <pre className="code muted" style={{ whiteSpace: 'pre-wrap' }}>
          {`kimbim versions remotes --add nicgit=${overview.clone_url}\nkimbim versions push nicgit\n# user: ${me.data?.slug ?? 'you'}  password: a token from Account`}
        </pre>
        <p>
          <Link to="/account">{t('account.tokens')}</Link>
        </p>
      </div>
    </div>
  )
}

export function ElementPanel({ guid, props, rev }: { guid: string; props: PropertyGroup[]; rev: string }) {
  const t = useT()
  const lang = useLang()
  const { api } = useProject()
  const history = useJob<{ history: { commit: string; kind: string; message: string; time: number; author: string }[] }>(
    api(`/elements/${encodeURIComponent(guid)}/history?rev=${encodeURIComponent(rev)}`),
  )
  const h0 = history.data
  return (
    <>
      {props.map((g, i) => (
        <section key={i}>
          <h3>{i === 0 ? ifcClass(g.name) : g.name}</h3>
          <dl className="kv">
            {g.values.map(([k, v], j) => (
              <div key={j} style={{ display: 'contents' }}>
                <dt>{k}</dt>
                <dd>{v}</dd>
              </div>
            ))}
          </dl>
        </section>
      ))}
      <h3>{t('viewer.history')}</h3>
      {h0?.pending && <JobWait progress={h0.progress} />}
      {h0 && !h0.pending && (
        <ul className="list small">
          {h0.data.history.map((h) => (
            <li key={h.commit}>
              <span className={`change ${h.kind}`}>{h.message}</span>
              <div className="muted">
                {h.author}, {when(h.time, lang)}
              </div>
            </li>
          ))}
        </ul>
      )}
    </>
  )
}

export function Overview() {
  const t = useT()
  const ctx = useProject()
  const { overview } = ctx
  const [params, setParams] = useSearchParams()
  const [sel, setSel] = useState<{ guid: string; props: PropertyGroup[] } | null>(null)
  const option = params.get('option') ?? overview.options.find((o) => o.is_default)?.name ?? overview.options[0]?.name
  const head = overview.options.find((o) => o.name === option)
  const latest = useApi<Version[]>(option ? ctx.api(`/versions?option=${encodeURIComponent(option)}&limit=1`) : null)
  if (overview.empty || !option || !head) return <EmptyProject />
  const version = latest.data?.[0] ?? null

  return (
    <div className="split">
      <Viewer model={ctx.api(`/versions/${head.head}/model.ifc`)} onSelect={(guid, props) => setSel(guid ? { guid, props } : null)}>
        <TitleBlock title={overview.project.full_name} option={option} version={version} />
      </Viewer>
      <aside className="side">
        {sel ? (
          <>
            <button type="button" className="btn-quiet" onClick={() => setSel(null)} style={{ paddingLeft: 0 }}>
              {t('common.back')}
            </button>
            <ElementPanel guid={sel.guid} props={sel.props} rev={head.head} />
          </>
        ) : (
          <>
            <h3>{t('overview.options')}</h3>
            <ul className="list">
              {overview.options.map((o) => (
                <li key={o.name}>
                  <button
                    type="button"
                    className="btn-quiet"
                    style={{ paddingLeft: 0, fontWeight: o.name === option ? 700 : 500 }}
                    onClick={() => setParams({ option: o.name })}
                  >
                    {o.label}
                  </button>{' '}
                  {o.is_agent && <span className="tag agent">{t('versions.ai')}</span>} {o.protected && <span className="tag">{t('settings.rules')}</span>}
                  <div className="small muted">{o.message}</div>
                  <Link className="small" to={`${ctx.base}/versions?option=${optionPath(o.name)}`}>
                    {t('rail.versions')}
                  </Link>
                </li>
              ))}
            </ul>
            <h3>{t('overview.latest')}</h3>
            <p className="small">{version?.message}</p>
            <dl className="kv">
              <dt>{t('overview.schema')}</dt>
              <dd>{overview.schema ?? ''}</dd>
              <dt>{t('rail.model')}</dt>
              <dd>{t('overview.elements', { n: overview.counts?.objects ?? 0 })}</dd>
              <dt>{t('overview.openProposals')}</dt>
              <dd>{overview.open_proposals}</dd>
              <dt>{t('overview.openIssues')}</dt>
              <dd>{overview.open_issues}</dd>
            </dl>
            <p style={{ marginTop: '1rem' }}>
              <a className="btn" href={ctx.api(`/versions/${head.head}/model.ifc`)} download>
                {t('overview.download')}
              </a>
            </p>
            <h3>{t('overview.clone')}</h3>
            <code className="small" style={{ overflowWrap: 'anywhere' }}>
              {overview.clone_url}
            </code>
            <p className="small muted" style={{ marginTop: '1rem' }}>
              {t('viewer.select')}
            </p>
          </>
        )}
      </aside>
    </div>
  )
}
