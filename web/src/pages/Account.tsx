// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import { api, useAction, useApi, useMe } from '../lib/api'
import { bytes, whenIso } from '../lib/format'

interface Token {
  id: string
  name: string
  last_used_at: string | null
  created_at: string
}

export function Account() {
  const t = useT()
  const lang = useLang()
  const me = useMe()
  const tokens = useApi<Token[]>('/api/me/tokens')
  const usage = useApi<{ kind: string; quantity: number }[]>('/api/me/usage')
  const [name, setName] = useState('')
  const [fresh, setFresh] = useState<string | null>(null)
  const [org, setOrg] = useState('')
  const create = useAction((n: string) => api<{ token: string }>('/api/me/tokens', { method: 'POST', json: { name: n } }), [['/api/me/tokens']])
  const revoke = useAction((id: string) => api(`/api/me/tokens/${id}`, { method: 'DELETE' }), [['/api/me/tokens']])
  const setLang = useAction((l: string) => api('/api/me', { method: 'PATCH', json: { lang: l } }), [['/api/me']])
  const ai = useApi<{ enabled: boolean; router: boolean }>(me.data ? `/api/accounts/${me.data.slug}/ai` : null)
  const setAi = useAction((enabled: boolean) => api(`/api/accounts/${me.data!.slug}/ai`, { method: 'PUT', json: { enabled } }), [[`/api/accounts/${me.data?.slug}/ai`]])
  const newOrg = useAction((slug: string) => api('/api/orgs', { method: 'POST', json: { slug } }), [['/api/me']])

  return (
    <main className="page">
      <h1>{t('account.title')}</h1>
      <p>
        {me.data?.display_name} <span className="muted">({me.data?.email})</span>
      </p>

      <section className="stack" style={{ marginTop: '2rem' }}>
        <h2>{t('account.tokens')}</h2>
        <p className="muted">{t('account.tokensHint')}</p>
        <form
          className="row"
          onSubmit={async (e) => {
            e.preventDefault()
            const r = await create.mutateAsync(name)
            setFresh(r.token)
            setName('')
          }}
        >
          <input aria-label={t('account.tokenName')} placeholder={t('account.tokenName')} value={name} onChange={(e) => setName(e.target.value)} required style={{ maxWidth: '20rem' }} />
          <button className="btn-primary">{t('account.createToken')}</button>
        </form>
        {fresh && (
          <div className="notice">
            <p>{t('account.tokenOnce')}</p>
            <div className="row">
              <code>{fresh}</code>
              <button type="button" className="btn-quiet" onClick={() => navigator.clipboard.writeText(fresh)}>
                {t('common.copy')}
              </button>
            </div>
          </div>
        )}
        {!!tokens.data?.length && (
          <table className="schedule">
            <tbody>
              {tokens.data.map((tk) => (
                <tr key={tk.id}>
                  <td>{tk.name}</td>
                  <td className="muted small">{whenIso(tk.created_at, lang)}</td>
                  <td className="muted small">{tk.last_used_at ? whenIso(tk.last_used_at, lang) : ''}</td>
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

      <section className="stack" style={{ marginTop: '2.5rem' }}>
        <h2>{t('account.language')}</h2>
        <div className="row">
          {(['en', 'es'] as const).map((l) => (
            <button key={l} type="button" className={lang === l ? 'btn-primary' : ''} onClick={() => setLang.mutate(l)}>
              {l === 'en' ? 'English' : 'Español'}
            </button>
          ))}
        </div>
      </section>

      {ai.data && (
        <section className="stack" style={{ marginTop: '2.5rem' }}>
          <h2>{t('account.ai')}</h2>
          <p className="muted">{t('account.aiHint')}</p>
          {!ai.data.router && <p className="notice warn">{t('account.aiNoRouter')}</p>}
          <label style={{ fontWeight: 400, display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
            <input type="checkbox" style={{ width: 'auto' }} checked={ai.data.enabled} onChange={(e) => setAi.mutate(e.target.checked)} />
            {ai.data.enabled ? t('account.aiOn') : t('account.aiOff')}
          </label>
        </section>
      )}

      <section className="stack" style={{ marginTop: '2.5rem' }}>
        <h2>{t('account.orgs')}</h2>
        <ul className="list">
          {me.data?.orgs.map((o) => (
            <li key={o.slug}>
              <Link to={`/explore`}>{o.display_name}</Link> <span className="muted">{t(`role.${o.role}` as 'role.admin')}</span>
            </li>
          ))}
        </ul>
        <form
          className="row"
          onSubmit={(e) => {
            e.preventDefault()
            newOrg.mutate(org)
            setOrg('')
          }}
        >
          <input aria-label={t('account.newOrg')} placeholder={t('account.newOrg')} value={org} onChange={(e) => setOrg(e.target.value.toLowerCase())} style={{ maxWidth: '20rem' }} />
          <button>{t('account.newOrg')}</button>
        </form>
        {newOrg.error && <p className="error">{newOrg.error.message}</p>}
      </section>

      {!!usage.data?.length && (
        <section className="stack" style={{ marginTop: '2.5rem' }}>
          <h2>{t('account.usage')}</h2>
          <dl className="kv">
            {usage.data.map((u) => (
              <div key={u.kind} style={{ display: 'contents' }}>
                <dt>{u.kind}</dt>
                <dd>{u.kind.endsWith('bytes') ? bytes(u.quantity) : u.quantity}</dd>
              </div>
            ))}
          </dl>
        </section>
      )}
    </main>
  )
}
