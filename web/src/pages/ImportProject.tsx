// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useT } from '../i18n'
import { ApiError, useMe } from '../lib/api'
import { OwnerSelect } from './NewProject'
import { JobWait } from '../components/States'

export function ImportProject() {
  const t = useT()
  const me = useMe()
  const nav = useNavigate()
  const [owner, setOwner] = useState(me.data?.slug ?? '')
  const [slug, setSlug] = useState('')
  const [file, setFile] = useState<File | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  async function submit(e: React.FormEvent) {
    e.preventDefault()
    if (!file) return
    setBusy(true)
    setError(null)
    const body = new FormData()
    body.set('owner', owner)
    body.set('slug', slug)
    body.set('file', file)
    try {
      const r = await fetch('/api/projects/import', { method: 'POST', body, credentials: 'same-origin' })
      const d = await r.json()
      if (!r.ok) throw new ApiError(r.status, d.error, d.message)
      // Wait for the engine to finish the import before opening the project.
      for (;;) {
        const j = await (await fetch(`/api/jobs/${d.job}`, { credentials: 'same-origin' })).json()
        if (j.status === 'done') break
        if (j.status === 'failed') throw new Error(j.error)
        await new Promise((res) => setTimeout(res, 1000))
      }
      nav(`/${d.project.full_name}`)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      setBusy(false)
    }
  }
  return (
    <main className="page page-narrow">
      <h1>{t('import.title')}</h1>
      <p className="muted">{t('import.hint')}</p>
      <form onSubmit={submit}>
        <div className="field">
          <label htmlFor="owner">{t('project.owner')}</label>
          <OwnerSelect value={owner} onChange={setOwner} />
        </div>
        <div className="field">
          <label htmlFor="slug">{t('project.name')}</label>
          <input id="slug" value={slug} onChange={(e) => setSlug(e.target.value.toLowerCase())} required />
        </div>
        <div className="field">
          <label htmlFor="file">{t('import.file')}</label>
          <input
            id="file"
            type="file"
            accept=".nic,.ifc"
            required
            onChange={(e) => {
              const f = e.target.files?.[0] ?? null
              setFile(f)
              if (f && !slug) setSlug(f.name.replace(/\.(kim|ifc)$/i, '').toLowerCase().replace(/[^a-z0-9._-]+/g, '-').replace(/^-+/, ''))
            }}
          />
        </div>
        {error && <p className="error" role="alert">{error}</p>}
        {busy ? <JobWait progress={0} label={t('import.working')} /> : <button className="btn-primary">{t('import.go')}</button>}
      </form>
    </main>
  )
}
