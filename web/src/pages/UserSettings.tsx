// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Link, NavLink, Outlet } from 'react-router-dom'
import { useLang, useStyle, useT } from '../i18n'
import type { Key } from '../i18n/en'
import { api, useAction, useApi, useMe } from '../lib/api'
import { bytes, whenIso } from '../lib/format'
import { Icon, type IconName } from '../components/Icon'
import { Info } from '../components/Info'
import { ErrorNote, Loading } from '../components/States'

const NAV: { to: string; key: Key; icon: IconName }[] = [
  { to: 'profile', key: 'account.profile', icon: 'user' },
  { to: 'access', key: 'account.access', icon: 'key' },
  { to: 'appearance', key: 'account.appearance', icon: 'globe' },
  { to: 'organizations', key: 'account.orgs', icon: 'building' },
  { to: 'ai', key: 'account.ai', icon: 'bot' },
  { to: 'billing', key: 'account.billing', icon: 'card' },
]

interface Token {
  id: string
  name: string
  last_used_at: string | null
  created_at: string
}

export function UserSettingsLayout() {
  const t = useT()
  return (
    <main className="settings-shell">
      <aside className="settings-side">
        <h2>{t('settings.user')}</h2>
        <nav className="settings-nav" aria-label={t('settings.user')}>
          {NAV.map((item) => (
            <NavLink key={item.to} to={item.to} end className={({ isActive }) => (isActive ? 'active' : undefined)}>
              <Icon name={item.icon} />
              <span>{t(item.key)}</span>
            </NavLink>
          ))}
        </nav>
      </aside>
      <div className="settings-main">
        <Outlet />
      </div>
    </main>
  )
}

function PageHead({ title, info }: { title: string; info?: string }) {
  return (
    <header className="page-head">
      <h1>
        {title}
        {info && <Info text={info} />}
      </h1>
    </header>
  )
}

export function ProfileSettings() {
  const t = useT()
  const me = useMe()
  const [userName, setUserName] = useState<string | null>(null)
  const rename = useAction((slug: string) => api('/api/me', { method: 'PATCH', json: { slug } }), [['/api/me']])
  const name = userName ?? me.data?.slug ?? ''
  const initial = (me.data?.display_name?.trim() || me.data?.slug || '?').slice(0, 1)
  return (
    <section>
      <PageHead title={t('account.profile')} />
      <div className="profile-head">
        <span className="avatar avatar-lg">{initial}</span>
        <div>
          <h2>{me.data?.display_name}</h2>
          <div className="muted">{me.data?.email}</div>
        </div>
      </div>
      <form
        onSubmit={async (e) => {
          e.preventDefault()
          if (userName === null) return
          await rename.mutateAsync(userName)
          setUserName(null)
        }}
      >
        <div className="field">
          <div className="label-row">
            <label htmlFor="user-name">{t('account.userName')}</label>
            <Info text={t('account.userNameHint')} />
          </div>
          <input
            id="user-name"
            value={name}
            onChange={(e) => setUserName(e.target.value.toLowerCase())}
            required
            style={{ maxWidth: '22rem' }}
          />
        </div>
        {rename.error && <p className="error" role="alert">{rename.error.message}</p>}
        <button className="btn-primary" disabled={userName === null || userName === me.data?.slug}>
          {t('account.rename')}
        </button>
      </form>
      <div className="form-block" style={{ marginTop: '2rem' }}>
        <h2>
          {t('onboarding.workProfile')} <Info text={t('onboarding.workProfileHint')} />
        </h2>
        <Link className="btn" to="/welcome">
          {t('onboarding.update')}
        </Link>
      </div>
    </section>
  )
}

export function AccessSettings() {
  const t = useT()
  const lang = useLang()
  const tokens = useApi<Token[]>('/api/me/tokens')
  const [name, setName] = useState('')
  const [fresh, setFresh] = useState<string | null>(null)
  const create = useAction((n: string) => api<{ token: string }>('/api/me/tokens', { method: 'POST', json: { name: n } }), [['/api/me/tokens']])
  const revoke = useAction((id: string) => api(`/api/me/tokens/${id}`, { method: 'DELETE' }), [['/api/me/tokens']])
  return (
    <section>
      <PageHead title={t('account.tokens')} info={t('account.tokensHint')} />
      <form
        className="row"
        style={{ marginBottom: '1.25rem' }}
        onSubmit={async (e) => {
          e.preventDefault()
          const r = await create.mutateAsync(name)
          setFresh(r.token)
          setName('')
        }}
      >
        <input
          aria-label={t('account.tokenName')}
          placeholder={t('account.tokenName')}
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
          style={{ maxWidth: '20rem' }}
        />
        <button className="btn-primary">{t('account.createToken')}</button>
      </form>
      {fresh && (
        <div className="notice" style={{ marginBottom: '1.25rem' }}>
          <p>{t('account.tokenOnce')}</p>
          <div className="row">
            <code>{fresh}</code>
            <button type="button" className="btn-quiet" onClick={() => navigator.clipboard.writeText(fresh)}>
              {t('common.copy')}
            </button>
          </div>
        </div>
      )}
      {tokens.isLoading && <Loading />}
      <ErrorNote error={tokens.error} />
      {tokens.data?.length === 0 && <p className="muted small">{t('account.noTokens')}</p>}
      {!!tokens.data?.length && (
        <table className="schedule">
          <thead>
            <tr>
              <th>{t('account.tokenName')}</th>
              <th>{t('account.created')}</th>
              <th>{t('account.lastUsed')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {tokens.data.map((tk) => (
              <tr key={tk.id}>
                <td>{tk.name}</td>
                <td className="muted small">{whenIso(tk.created_at, lang)}</td>
                <td className="muted small">{tk.last_used_at ? whenIso(tk.last_used_at, lang) : '—'}</td>
                <td className="num">
                  <button type="button" className="btn-quiet" onClick={() => revoke.mutate(tk.id)}>
                    {t('account.revoke')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  )
}

export function AppearanceSettings() {
  const t = useT()
  const lang = useLang()
  const style = useStyle()
  const setLang = useAction((l: string) => api('/api/me', { method: 'PATCH', json: { lang: l } }), [['/api/me']])
  const setStyle = useAction((s: string) => api('/api/me', { method: 'PATCH', json: { style: s } }), [['/api/me']])
  return (
    <section>
      <PageHead title={t('account.appearance')} />
      <h2>{t('account.language')}</h2>
      <div className="row">
        {(['en', 'es'] as const).map((l) => (
          <button key={l} type="button" data-lang={l} aria-pressed={lang === l} className={lang === l ? 'btn-primary' : ''} onClick={() => setLang.mutate(l)}>
            {l === 'en' ? 'English' : 'Español'}
          </button>
        ))}
      </div>
      <h2 style={{ marginTop: '2rem' }}>
        {t('account.style')} <Info text={t('account.styleHint')} />
      </h2>
      <div className="row">
        {(['git', 'architecture'] as const).map((s) => (
          <button key={s} type="button" data-style={s} aria-pressed={style === s} className={style === s ? 'btn-primary' : ''} onClick={() => setStyle.mutate(s)}>
            {s === 'git' ? t('account.styleGit') : t('account.styleArchitecture')}
          </button>
        ))}
      </div>
    </section>
  )
}

export function OrganizationsSettings() {
  const t = useT()
  const me = useMe()
  const [org, setOrg] = useState('')
  const newOrg = useAction((slug: string) => api('/api/orgs', { method: 'POST', json: { slug } }), [['/api/me']])
  return (
    <section>
      <PageHead title={t('account.orgs')} />
      <ul className="list">
        {me.data?.orgs.map((o) => (
          <li key={o.slug} className="row" style={{ justifyContent: 'space-between' }}>
            <span>
              <Icon name="building" />{' '}
              <Link to={`/${o.slug}`}>
                <strong>{o.display_name}</strong>
              </Link>{' '}
              <span className="muted small" style={{ marginLeft: '0.35rem' }}>{o.slug}</span>
            </span>
            <span className="tag">{t(`role.${o.role}` as 'role.admin')}</span>
          </li>
        ))}
        {me.data?.orgs.length === 0 && <li className="muted small">{t('account.noOrgs')}</li>}
      </ul>
      <form
        className="row"
        style={{ marginTop: '1.25rem' }}
        onSubmit={(e) => {
          e.preventDefault()
          newOrg.mutate(org)
          setOrg('')
        }}
      >
        <input
          aria-label={t('account.newOrg')}
          placeholder={t('account.newOrg')}
          value={org}
          onChange={(e) => setOrg(e.target.value.toLowerCase())}
          style={{ maxWidth: '20rem' }}
        />
        <button>{t('account.newOrg')}</button>
      </form>
      {newOrg.error && <p className="error">{newOrg.error.message}</p>}
    </section>
  )
}

export function AiSettings() {
  const t = useT()
  const me = useMe()
  const ai = useApi<{ enabled: boolean; router: boolean }>(me.data ? `/api/accounts/${me.data.slug}/ai` : null)
  const setAi = useAction(
    (enabled: boolean) => api(`/api/accounts/${me.data!.slug}/ai`, { method: 'PUT', json: { enabled } }),
    [[`/api/accounts/${me.data?.slug}/ai`]],
  )
  return (
    <section>
      <PageHead title={t('account.ai')} info={t('account.aiHint')} />
      {ai.isLoading && <Loading />}
      <ErrorNote error={ai.error} />
      {ai.data && (
        <>
          {!ai.data.router && <p className="notice warn">{t('account.aiNoRouter')}</p>}
          <label className="row" style={{ fontWeight: 400 }}>
            <input type="checkbox" style={{ width: 'auto' }} checked={ai.data.enabled} onChange={(e) => setAi.mutate(e.target.checked)} />
            {ai.data.enabled ? t('account.aiOn') : t('account.aiOff')}
          </label>
        </>
      )}
    </section>
  )
}

export function BillingSettings() {
  const t = useT()
  const storage = useApi<{ used: number; limit: number; project_limit: number; file_limit: number }>('/api/me/storage')
  const usage = useApi<{ kind: string; quantity: number }[]>('/api/me/usage')
  const pct = storage.data && storage.data.limit > 0 ? Math.min(100, (storage.data.used / storage.data.limit) * 100) : 0
  return (
    <section>
      <PageHead title={t('account.billing')} />
      <h2>{t('account.storage')}</h2>
      {storage.isLoading && <Loading />}
      <ErrorNote error={storage.error} />
      {storage.data && (
        <>
          <div className="meter-row">
            <div className="progress" aria-hidden>
              <span style={{ width: `${Math.max(pct, 2)}%` }} />
            </div>
            <span className="small">
              {t('account.storageUsed', { used: bytes(storage.data.used), limit: bytes(storage.data.limit) })}
            </span>
          </div>
          <p className="small muted">
            {t('account.storageLimits', { project: bytes(storage.data.project_limit), file: bytes(storage.data.file_limit) })}
          </p>
        </>
      )}
      <h2 style={{ marginTop: '2rem' }}>{t('account.usage')}</h2>
      {usage.isLoading && <Loading />}
      <ErrorNote error={usage.error} />
      {usage.data?.length === 0 && <p className="muted small">{t('account.noUsage')}</p>}
      {!!usage.data?.length && (
        <dl className="kv">
          {usage.data.map((u) => (
            <div key={u.kind} style={{ display: 'contents' }}>
              <dt>{u.kind}</dt>
              <dd>{u.kind.endsWith('bytes') ? bytes(u.quantity) : u.quantity}</dd>
            </div>
          ))}
        </dl>
      )}
    </section>
  )
}
