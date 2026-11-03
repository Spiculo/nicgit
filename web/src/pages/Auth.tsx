// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useT } from '../i18n'
import { api, ApiError } from '../lib/api'

function useSubmit(path: string, body: () => unknown) {
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await api(path, { method: 'POST', json: body() })
      window.location.assign('/')
    } catch (err) {
      setError(err instanceof ApiError ? err.message : String(err))
      setBusy(false)
    }
  }
  return { error, busy, submit }
}

export function Login() {
  const t = useT()
  const [login, setLogin] = useState('')
  const [password, setPassword] = useState('')
  const { error, busy, submit } = useSubmit('/api/auth/login', () => ({ login, password }))
  return (
    <main className="page page-narrow">
      <h1>{t('auth.signIn')}</h1>
      <form onSubmit={submit}>
        <div className="field">
          <label htmlFor="login">{t('auth.login')}</label>
          <input id="login" autoComplete="username" value={login} onChange={(e) => setLogin(e.target.value)} required />
        </div>
        <div className="field">
          <label htmlFor="password">{t('auth.password')}</label>
          <input id="password" type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} required />
        </div>
        {error && <p className="error" role="alert">{error}</p>}
        <button className="btn-primary" disabled={busy}>
          {t('auth.signIn')}
        </button>
      </form>
      <p className="muted" style={{ marginTop: '1.5rem' }}>
        {t('auth.noAccount')} <Link to="/signup">{t('auth.signUp')}</Link>
      </p>
    </main>
  )
}

export function Signup() {
  const t = useT()
  const [f, setF] = useState({ slug: '', email: '', password: '', display_name: '' })
  const { error, busy, submit } = useSubmit('/api/auth/signup', () => f)
  const set = (k: keyof typeof f) => (e: React.ChangeEvent<HTMLInputElement>) => setF({ ...f, [k]: e.target.value })
  return (
    <main className="page page-narrow">
      <h1>{t('auth.signUp')}</h1>
      <form onSubmit={submit}>
        <div className="field">
          <label htmlFor="name">{t('auth.name')}</label>
          <input id="name" autoComplete="name" value={f.display_name} onChange={set('display_name')} />
        </div>
        <div className="field">
          <label htmlFor="slug">{t('auth.slug')}</label>
          <input id="slug" autoComplete="username" value={f.slug} onChange={set('slug')} required pattern="[a-z0-9][a-z0-9._\-]*" />
          <div className="hint">{t('project.nameHint')}</div>
        </div>
        <div className="field">
          <label htmlFor="email">{t('auth.email')}</label>
          <input id="email" type="email" autoComplete="email" value={f.email} onChange={set('email')} required />
        </div>
        <div className="field">
          <label htmlFor="password">{t('auth.password')}</label>
          <input id="password" type="password" autoComplete="new-password" minLength={10} value={f.password} onChange={set('password')} required />
          <div className="hint">{t('auth.passwordHint')}</div>
        </div>
        {error && <p className="error" role="alert">{error}</p>}
        <button className="btn-primary" disabled={busy}>
          {t('auth.signUp')}
        </button>
      </form>
      <p className="muted" style={{ marginTop: '1.5rem' }}>
        {t('auth.haveAccount')} <Link to="/login">{t('auth.signIn')}</Link>
      </p>
    </main>
  )
}
