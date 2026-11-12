// SPDX-License-Identifier: AGPL-3.0-or-later
import { useSearchParams } from 'react-router-dom'
import { useT } from '../i18n'

const ERRORS = {
  denied: 'auth.error.denied',
  state: 'auth.error.state',
  exchange: 'auth.error.exchange',
  token: 'auth.error.token',
  unverified: 'auth.error.unverified',
  conflict: 'auth.error.conflict',
  signup_closed: 'auth.error.signupClosed',
  not_configured: 'auth.error.notConfigured',
  server: 'auth.error.server',
} as const

export function Login() {
  const t = useT()
  const [params] = useSearchParams()
  const code = params.get('error')
  const key = code && Object.hasOwn(ERRORS, code) ? ERRORS[code as keyof typeof ERRORS] : code ? ERRORS.server : null
  return (
    <main className="page page-narrow">
      <h1>{t('auth.signIn')}</h1>
      {key && <p className="error" role="alert">{t(key)}</p>}
      {/* A full page load: the server sends the browser on to Google. */}
      <a className="btn btn-primary" style={{ textDecoration: 'none' }} href="/api/auth/google/start">
        {t('auth.google')}
      </a>
    </main>
  )
}
