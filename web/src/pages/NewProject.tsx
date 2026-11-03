// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useT } from '../i18n'
import { api, ApiError, useMe } from '../lib/api'
import type { Project } from '../lib/types'

export function OwnerSelect({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const me = useMe()
  const owners = [me.data?.slug ?? '', ...(me.data?.orgs.filter((o) => o.role === 'lead' || o.role === 'admin').map((o) => o.slug) ?? [])]
  return (
    <select id="owner" value={value} onChange={(e) => onChange(e.target.value)}>
      {owners.map((o) => (
        <option key={o} value={o}>
          {o}
        </option>
      ))}
    </select>
  )
}

export function NewProject() {
  const t = useT()
  const me = useMe()
  const nav = useNavigate()
  const [owner, setOwner] = useState(me.data?.slug ?? '')
  const [slug, setSlug] = useState('')
  const [description, setDescription] = useState('')
  const [visibility, setVisibility] = useState<'private' | 'public'>('private')
  const [error, setError] = useState<string | null>(null)
  async function submit(e: React.FormEvent) {
    e.preventDefault()
    try {
      const p = await api<Project>('/api/projects', { method: 'POST', json: { owner, slug, description, visibility } })
      nav(`/${p.full_name}`)
    } catch (err) {
      setError(err instanceof ApiError ? err.message : String(err))
    }
  }
  return (
    <main className="page page-narrow">
      <h1>{t('nav.newProject')}</h1>
      <form onSubmit={submit}>
        <div className="field">
          <label htmlFor="owner">{t('project.owner')}</label>
          <OwnerSelect value={owner} onChange={setOwner} />
        </div>
        <div className="field">
          <label htmlFor="slug">{t('project.name')}</label>
          <input id="slug" value={slug} onChange={(e) => setSlug(e.target.value.toLowerCase())} required />
          <div className="hint">{t('project.nameHint')}</div>
        </div>
        <div className="field">
          <label htmlFor="description">{t('project.description')}</label>
          <input id="description" value={description} onChange={(e) => setDescription(e.target.value)} />
        </div>
        <fieldset className="field" style={{ border: 0, padding: 0 }}>
          <legend style={{ fontWeight: 560, marginBottom: '0.25rem' }}>{t('project.visibility')}</legend>
          {(['private', 'public'] as const).map((v) => (
            <label key={v} style={{ fontWeight: 400, display: 'flex', gap: '0.5rem' }}>
              <input type="radio" style={{ width: 'auto' }} checked={visibility === v} onChange={() => setVisibility(v)} />
              {v === 'private' ? t('project.private') : t('project.public')}
            </label>
          ))}
        </fieldset>
        {error && <p className="error" role="alert">{error}</p>}
        <button className="btn-primary">{t('project.create')}</button>
      </form>
    </main>
  )
}
