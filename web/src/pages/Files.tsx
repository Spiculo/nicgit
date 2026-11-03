// SPDX-License-Identifier: AGPL-3.0-or-later
import { Link, useSearchParams } from 'react-router-dom'
import { useT } from '../i18n'
import { useApi } from '../lib/api'
import { bytes } from '../lib/format'
import { useProject } from './ProjectLayout'
import { ErrorNote, Loading } from '../components/States'

interface Listing {
  commit: string
  path: string
  entries: { name: string; type: 'dir' | 'file'; size: number | null }[]
}

export function Files() {
  const t = useT()
  const ctx = useProject()
  const [params] = useSearchParams()
  const rev = params.get('rev') ?? ctx.overview.project.default_option
  const path = params.get('path') ?? ''
  const file = params.get('file')
  const list = useApi<Listing>(file ? null : ctx.api(`/files?rev=${encodeURIComponent(rev)}&path=${encodeURIComponent(path)}`))
  const blob = useApi<unknown>(file ? ctx.api(`/blob?rev=${encodeURIComponent(rev)}&path=${encodeURIComponent(file)}`) : null)
  const href = (p: string, isFile = false) => `${ctx.base}/files?rev=${encodeURIComponent(rev)}&${isFile ? 'file' : 'path'}=${encodeURIComponent(p)}`
  const parts = (file ?? path).split('/').filter(Boolean)
  return (
    <main className="page">
      <h1>{t('files.title')}</h1>
      <p className="muted">{t('files.recordsHint')}</p>
      <p className="crumbs small">
        <Link to={href('')}>{rev}</Link>
        {parts.map((p, i) => (
          <span key={i}>
            <span className="sep"> / </span>
            <Link to={href(parts.slice(0, i + 1).join('/'), file !== null && i === parts.length - 1)}>{p}</Link>
          </span>
        ))}
      </p>
      {(list.isLoading || blob.isLoading) && <Loading />}
      <ErrorNote error={list.error ?? blob.error} />
      {list.data && (
        <table className="schedule">
          <tbody>
            {list.data.entries.map((e) => (
              <tr key={e.name}>
                <td>
                  <Link to={href(path ? `${path}/${e.name}` : e.name, e.type === 'file')}>
                    {e.name}
                    {e.type === 'dir' ? '/' : ''}
                  </Link>
                </td>
                <td className="num muted small">{e.size !== null && e.size !== undefined ? bytes(e.size) : ''}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {blob.data !== undefined && blob.data !== null && (
        <pre className="code" style={{ background: 'var(--sheet)', border: '1px solid var(--grid)', padding: '1rem', overflow: 'auto' }}>
          {JSON.stringify(blob.data, null, 2)}
        </pre>
      )}
    </main>
  )
}
