// SPDX-License-Identifier: AGPL-3.0-or-later
import { Link } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import { optionLabel, shortId, when } from '../lib/format'
import { JobDownload } from '../components/JobDownload'
import { useProject } from './ProjectLayout'

export function Preview() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const latest = ctx.overview.latest
  const option = ctx.overview.project.default_option
  return (
    <main className="page">
      <header className="page-head">
        <h1>{t('preview.title')}</h1>
      </header>
      {latest ? (
        <section>
          <h2>{latest.message}</h2>
          <p className="muted small">
            {optionLabel(option)} · {latest.author} · {when(latest.time, lang)} · <code>{shortId(latest.id)}</code>
          </p>
          <div className="preview-actions">
            <Link className="btn btn-primary" to={`${ctx.base}/compare`}>
              {t('preview.openDiff')}
            </Link>
            <Link className="btn" to={`${ctx.base}/files`}>
              {t('preview.browse')}
            </Link>
            <a className="btn" href={ctx.api(`/versions/${latest.id}/model.ifc`)} download>
              {t('overview.download')}
            </a>
            <JobDownload href={ctx.api('/export.nic?scope=full')}>.nic</JobDownload>
            <a className="btn" href={ctx.api(`/archive?rev=${encodeURIComponent(latest.id)}`)} download>
              {t('overview.source')}
            </a>
            <a className="btn" href={ctx.api('/issues/export.bcfzip')} download>
              BCF
            </a>
          </div>
        </section>
      ) : (
        <div className="empty">
          <p>{t('preview.empty')}</p>
          <Link to={ctx.base}>{t('nav.overview')}</Link>
        </div>
      )}
    </main>
  )
}
