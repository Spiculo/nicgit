// SPDX-License-Identifier: AGPL-3.0-or-later
import { Link } from 'react-router-dom'
import { useT } from '../i18n'
import { useProject } from './ProjectLayout'

export function Preview() {
  const t = useT()
  const ctx = useProject()
  const latest = ctx.overview.latest
  return (
    <main className="page stack">
      <h1>{t('preview.title')}</h1>
      <p className="muted">{t('preview.intro')}</p>
      <div className="row">
        <Link className="btn btn-primary" to={`${ctx.base}/compare`}>
          {t('preview.openDiff')}
        </Link>
        <Link className="btn" to={`${ctx.base}/files`}>
          {t('preview.browse')}
        </Link>
        {latest && (
          <a className="btn" href={ctx.api(`/versions/${latest.id}/model.ifc`)} download>
            {t('overview.download')}
          </a>
        )}
      </div>
      <section id="diff">
        <h2>{t('preview.current')}</h2>
        <p>{t('preview.currentBody')}</p>
      </section>
      <section id="plan">
        <h2>{t('preview.next')}</h2>
        <ul className="list">
          <li>{t('preview.nextChanged')}</li>
          <li>{t('preview.nextSelected')}</li>
          <li>{t('preview.nextLinks')}</li>
          <li>{t('preview.nextDownloads')}</li>
        </ul>
      </section>
      <section id="files">
        <h2>{t('preview.files')}</h2>
        <p>{t('preview.filesBody')}</p>
        <Link to={`${ctx.base}/files`}>{t('rail.files')}</Link>
      </section>
    </main>
  )
}
