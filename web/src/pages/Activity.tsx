// SPDX-License-Identifier: AGPL-3.0-or-later
import { Link } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import { useApi } from '../lib/api'
import { whenIso } from '../lib/format'
import { useProject } from './ProjectLayout'
import { ErrorNote, Loading } from '../components/States'

interface Item {
  id: number
  actor: string | null
  kind: string
  data: Record<string, unknown>
  created_at: string
}

export function Activity() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const feed = useApi<Item[]>(ctx.api('/activity'))
  const line = (a: Item) => {
    switch (a.kind) {
      case 'push':
        return t('activity.push', { n: (a.data.updates as unknown[]).length })
      case 'project.created':
        return t('activity.created')
      case 'proposal.opened':
        return <Link to={`${ctx.base}/proposals/${a.data.number}`}>{t('activity.proposal', { n: String(a.data.number) })}</Link>
      case 'proposal.combined':
        return <Link to={`${ctx.base}/proposals/${a.data.number}`}>{t('activity.combined', { n: String(a.data.number) })}</Link>
      case 'issue.opened':
        return <Link to={`${ctx.base}/issues/${a.data.number}`}>{t('activity.issue', { n: String(a.data.number) })}</Link>
      case 'milestone.created':
        return t('activity.milestone', { tag: String(a.data.tag) })
      default:
        return a.kind
    }
  }
  return (
    <main className="page">
      <h1>{t('activity.title')}</h1>
      {feed.isLoading && <Loading />}
      <ErrorNote error={feed.error} />
      <ul className="list">
        {feed.data?.map((a) => (
          <li key={a.id}>
            <strong>{a.actor ?? '—'}</strong> {line(a)} <span className="small muted">{whenIso(a.created_at, lang)}</span>
          </li>
        ))}
      </ul>
    </main>
  )
}
