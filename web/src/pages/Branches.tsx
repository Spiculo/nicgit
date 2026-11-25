// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState, type FormEvent } from 'react'
import { useLang, useT } from '../i18n'
import { api, useAction, useApi } from '../lib/api'
import type { OptionInfo } from '../lib/types'
import { optionLabel, optionPath, shortId, when } from '../lib/format'
import { useProject } from './ProjectLayout'
import { ErrorNote, Loading } from '../components/States'

export function Branches() {
  const t = useT()
  const lang = useLang()
  const ctx = useProject()
  const key = ctx.api('/options')
  const list = useApi<OptionInfo[]>(key)
  const [name, setName] = useState('')
  const [from, setFrom] = useState(ctx.overview.project.default_option)
  const [error, setError] = useState<string | null>(null)

  const create = useAction(
    () => api(key, { method: 'POST', json: { name, from } }),
    [[key], [ctx.api('/overview')]],
  )
  const remove = useAction(
    (option: string) => api(ctx.api(`/branches/${optionPath(option)}`), { method: 'DELETE' }),
    [[key], [ctx.api('/overview')]],
  )
  const setDefault = useAction(
    (option: string) => api(ctx.api(), { method: 'PATCH', json: { default_option: option } }),
    [[key], [ctx.api('/overview')]],
  )

  const options = list.data ?? []
  const submit = (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    if (!name.trim()) return
    create.mutate(undefined, {
      onSuccess: () => setName(''),
      onError: (e) => setError(e.message),
    })
  }
  const drop = (o: OptionInfo) => {
    if (!window.confirm(t('branches.deleteConfirm', { name: optionLabel(o.name) }))) return
    remove.mutate(o.name, { onError: (e) => setError(e.message) })
  }
  const makeDefault = (o: OptionInfo) => {
    setError(null)
    setDefault.mutate(o.name, { onError: (e) => setError(e.message) })
  }

  return (
    <main className="page stack">
      <h1>{t('branches.title')}</h1>
      <p className="muted">{t('branches.hint')}</p>
      {list.isLoading && <Loading />}
      <ErrorNote error={list.error} />
      {error && <p className="error" role="alert">{error}</p>}

      {!!options.length && (
        <table className="schedule">
          <thead>
            <tr>
              <th>{t('branches.name')}</th>
              <th>{t('branches.head')}</th>
              <th>{t('branches.author')}</th>
              <th>{t('branches.updated')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {options.map((o) => (
              <tr key={o.name}>
                <td>
                  <strong>{optionLabel(o.name)}</strong>
                  <div className="small muted">
                    <code>{o.name}</code>
                    {o.is_default && <span className="badge">{t('branches.default')}</span>}
                    {o.protected && <span className="badge">{t('branches.protected')}</span>}
                    {o.is_agent && <span className="badge">agent</span>}
                  </div>
                </td>
                <td>
                  <code>{shortId(o.head)}</code>
                  {o.message && <div className="small muted">{o.message}</div>}
                </td>
                <td className="small">{o.author ?? ''}</td>
                <td className="small">{when(o.time, lang)}</td>
                <td className="num">
                  {!o.is_default && ctx.can('admin') && (
                    <button className="btn btn-small" onClick={() => makeDefault(o)} disabled={setDefault.isPending}>
                      {t('branches.setDefault')}
                    </button>
                  )}{' '}
                  {!o.is_default && ctx.can('member') && (!o.protected || ctx.can('lead')) && (
                    <button className="btn btn-small btn-danger" onClick={() => drop(o)} disabled={remove.isPending}>
                      {t('branches.delete')}
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {ctx.can('member') && (
        <form className="card stack" onSubmit={submit}>
          <h2>{t('branches.create')}</h2>
          <label>
            {t('branches.name')}
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Scheme C" required />
          </label>
          <label>
            {t('branches.from')}
            <select value={from} onChange={(e) => setFrom(e.target.value)}>
              {options.map((o) => (
                <option key={o.name} value={o.name}>
                  {optionLabel(o.name)}
                </option>
              ))}
            </select>
          </label>
          <button className="btn btn-primary" disabled={create.isPending || !name.trim()}>
            {t('branches.create')}
          </button>
        </form>
      )}
    </main>
  )
}
