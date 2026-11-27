// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Outlet, useNavigate } from 'react-router-dom'
import { useT } from '../i18n'
import { api, useAction, useApi } from '../lib/api'
import type { Role } from '../lib/types'
import { NotFound } from '../components/States'
import { useProject } from './ProjectLayout'

const ROLES: Role[] = ['viewer', 'reviewer', 'member', 'lead', 'admin']
const MODES = ['auto', 'whole:ours', 'whole:theirs', 'granular'] as const

interface Rule {
  option: string
  required_approvals: number
  required_roles: string[]
  allowed_modes: string[]
  required_checks: string[]
  protected_settings: string[]
  dismiss_on_push: boolean
}

function PageHead({ title }: { title: string }) {
  return (
    <header className="page-head">
      <h1>{title}</h1>
    </header>
  )
}

export function ProjectSettings() {
  const ctx = useProject()
  if (!ctx.can('admin')) return <NotFound />
  return (
    <main className="page page-settings">
      <Outlet />
    </main>
  )
}

export function SettingsGeneral() {
  const t = useT()
  const ctx = useProject()
  const p = ctx.overview.project
  const [description, setDescription] = useState(p.description)
  const [visibility, setVisibility] = useState(p.visibility)
  const [def, setDef] = useState(p.default_option)
  const save = useAction(
    () => api(ctx.api(), { method: 'PATCH', json: { description, visibility, default_option: def } }),
    [[ctx.api('/overview')]],
  )
  return (
    <section>
      <PageHead title={t('settings.general')} />
      <form
        onSubmit={(e) => {
          e.preventDefault()
          save.mutate(undefined)
        }}
      >
        <div className="field">
          <label htmlFor="d">{t('project.description')}</label>
          <input id="d" value={description} onChange={(e) => setDescription(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="v">{t('project.visibility')}</label>
          <select id="v" value={visibility} onChange={(e) => setVisibility(e.target.value as 'private' | 'public')}>
            <option value="private">{t('project.private')}</option>
            <option value="public">{t('project.public')}</option>
          </select>
        </div>
        <div className="field">
          <label htmlFor="o">{t('settings.defaultOption')}</label>
          <select id="o" value={def} onChange={(e) => setDef(e.target.value)}>
            {[def, ...ctx.overview.options.map((o) => o.name).filter((n) => n !== def)].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </div>
        <div className="row">
          <button className="btn-primary">{t('settings.save')}</button>
          {save.isSuccess && <span className="muted">{t('settings.saved')}</span>}
        </div>
        {save.error && <p className="error">{save.error.message}</p>}
      </form>
    </section>
  )
}

export function SettingsPeople() {
  const t = useT()
  const ctx = useProject()
  const members = useApi<{ slug: string; role: Role; via: string }[]>(ctx.api('/members'))
  const [who, setWho] = useState('')
  const [role, setRole] = useState<Role>('reviewer')
  const key = [ctx.api('/members')]
  const set = useAction((v: { who: string; role: Role }) => api(ctx.api(`/members/${v.who}`), { method: 'PUT', json: { role: v.role } }), [key])
  const remove = useAction((w: string) => api(ctx.api(`/members/${w}`), { method: 'DELETE' }), [key])
  return (
    <section>
      <PageHead title={t('settings.members')} />
      <form
        className="row"
        style={{ marginBottom: '1.25rem' }}
        onSubmit={(e) => {
          e.preventDefault()
          set.mutate({ who, role })
          setWho('')
        }}
      >
        <input aria-label={t('auth.slug')} placeholder={t('auth.slug')} value={who} onChange={(e) => setWho(e.target.value)} style={{ maxWidth: '14rem' }} required />
        <select aria-label={t('settings.role')} value={role} onChange={(e) => setRole(e.target.value as Role)} style={{ width: 'auto' }}>
          {ROLES.map((r) => (
            <option key={r} value={r}>
              {t(`role.${r}`)}
            </option>
          ))}
        </select>
        <button className="btn-primary">{t('settings.addMember')}</button>
      </form>
      {members.isLoading && <p className="muted">{t('common.loading')}</p>}
      {members.error && <p className="notice bad">{members.error.message}</p>}
      {members.data?.length === 0 && <p className="muted small">{t('settings.noMembers')}</p>}
      {!!members.data?.length && (
        <table className="schedule">
          <thead>
            <tr>
              <th>{t('auth.slug')}</th>
              <th>{t('settings.role')}</th>
              <th>{t('settings.via')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {members.data.map((m) => (
              <tr key={`${m.slug}-${m.via}`}>
                <td>{m.slug}</td>
                <td>{t(`role.${m.role}`)}</td>
                <td className="muted small">{m.via === 'project' ? t('settings.viaProject') : t('settings.viaOrg')}</td>
                <td className="num">
                  {m.via === 'project' && (
                    <button type="button" className="btn-quiet" onClick={() => remove.mutate(m.slug)}>
                      {t('common.remove')}
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {set.error && <p className="error">{set.error.message}</p>}
    </section>
  )
}

function RuleEditor({ rule, onSave, onRemove }: { rule: Rule; onSave: (r: Rule) => void; onRemove?: () => void }) {
  const t = useT()
  const [r, setR] = useState(rule)
  const toggle = (list: string[], v: string) => (list.includes(v) ? list.filter((x) => x !== v) : [...list, v])
  return (
    <form
      className="stack rule"
      onSubmit={(e) => {
        e.preventDefault()
        onSave(r)
      }}
    >
      <div className="row">
        <div className="field" style={{ flex: 1 }}>
          <label>{t('settings.ruleOption')}</label>
          <input value={r.option} onChange={(e) => setR({ ...r, option: e.target.value })} required />
        </div>
        <div className="field">
          <label>{t('settings.ruleApprovals')}</label>
          <input type="number" min={0} max={20} value={r.required_approvals} onChange={(e) => setR({ ...r, required_approvals: Number(e.target.value) })} style={{ width: '6rem' }} />
        </div>
      </div>
      <fieldset style={{ border: 0, padding: 0 }}>
        <legend style={{ fontWeight: 560 }}>{t('settings.ruleRoles')}</legend>
        <div className="row">
          {ROLES.slice(1).map((role) => (
            <label key={role} style={{ fontWeight: 400, display: 'flex', gap: '0.35rem' }}>
              <input type="checkbox" style={{ width: 'auto' }} checked={r.required_roles.includes(role)} onChange={() => setR({ ...r, required_roles: toggle(r.required_roles, role) })} />
              {t(`role.${role}`)}
            </label>
          ))}
        </div>
      </fieldset>
      <fieldset style={{ border: 0, padding: 0 }}>
        <legend style={{ fontWeight: 560 }}>{t('settings.ruleModes')}</legend>
        <div className="row">
          {MODES.map((m) => (
            <label key={m} style={{ fontWeight: 400, display: 'flex', gap: '0.35rem' }}>
              <input type="checkbox" style={{ width: 'auto' }} checked={r.allowed_modes.includes(m)} onChange={() => setR({ ...r, allowed_modes: toggle(r.allowed_modes, m) })} />
              {m === 'auto' ? t('proposals.modeAuto') : m === 'granular' ? t('proposals.modeGranular') : m === 'whole:ours' ? t('proposals.modeOurs', { target: r.option }) : t('proposals.modeTheirs', { source: '…' })}
            </label>
          ))}
        </div>
      </fieldset>
      <div className="field">
        <label>{t('settings.ruleSettings')}</label>
        <input value={r.protected_settings.join(', ')} onChange={(e) => setR({ ...r, protected_settings: e.target.value.split(',').map((s) => s.trim()).filter(Boolean) })} />
      </div>
      <label style={{ fontWeight: 400, display: 'flex', gap: '0.35rem' }}>
        <input type="checkbox" style={{ width: 'auto' }} checked={r.dismiss_on_push} onChange={(e) => setR({ ...r, dismiss_on_push: e.target.checked })} />
        {t('settings.ruleDismiss')}
      </label>
      <div className="row">
        <button className="btn-primary">{t('settings.save')}</button>
        {onRemove && (
          <button type="button" className="btn-danger" onClick={onRemove}>
            {t('common.remove')}
          </button>
        )}
      </div>
    </form>
  )
}

const NEW_RULE: Rule = {
  option: 'main',
  required_approvals: 1,
  required_roles: ['lead'],
  allowed_modes: ['auto', 'whole:ours', 'whole:theirs', 'granular'],
  required_checks: [],
  protected_settings: ['ifc_export', 'georeference', 'title_block_templates', 'units', 'model_resolution', 'precommit_checks'],
  dismiss_on_push: true,
}

export function SettingsRules() {
  const t = useT()
  const ctx = useProject()
  const rules = useApi<{ rules: Rule[]; settings_keys: string[] }>(ctx.api('/rules'))
  const key = [ctx.api('/rules')]
  const save = useAction((r: Rule) => api(ctx.api(`/rules/${encodeURIComponent(r.option)}`), { method: 'PUT', json: r }), [key])
  const remove = useAction((o: string) => api(ctx.api(`/rules/${encodeURIComponent(o)}`), { method: 'DELETE' }), [key])
  const [adding, setAdding] = useState(false)
  return (
    <section>
      <PageHead title={t('settings.rules')} />
      {rules.isLoading && <p className="muted">{t('common.loading')}</p>}
      {rules.error && <p className="notice bad">{rules.error.message}</p>}
      {rules.data?.rules.length === 0 && <p className="muted small">{t('settings.noRules')}</p>}
      {rules.data?.rules.map((r) => (
        <RuleEditor key={r.option} rule={r} onSave={(x) => save.mutate(x)} onRemove={() => remove.mutate(r.option)} />
      ))}
      {adding ? (
        <RuleEditor
          rule={NEW_RULE}
          onSave={(x) => {
            save.mutate(x)
            setAdding(false)
          }}
        />
      ) : (
        <button type="button" style={{ marginTop: '1.25rem' }} onClick={() => setAdding(true)}>
          {t('settings.addRule')}
        </button>
      )}
      {save.error && <p className="error">{save.error.message}</p>}
    </section>
  )
}

export function SettingsWebhooks() {
  const t = useT()
  return (
    <section>
      <PageHead title={t('config.webhooks')} />
      <div className="empty">
        <p>{t('config.empty')}</p>
      </div>
    </section>
  )
}

export function SettingsAudit() {
  const t = useT()
  return (
    <section>
      <PageHead title={t('config.audit')} />
      <div className="empty">
        <p>{t('config.empty')}</p>
      </div>
    </section>
  )
}

export function SettingsDanger() {
  const t = useT()
  const ctx = useProject()
  const nav = useNavigate()
  const [confirm, setConfirm] = useState('')
  return (
    <section>
      <PageHead title={t('settings.danger')} />
      <p>{t('settings.deleteConfirm')}</p>
      <div className="row">
        <input value={confirm} onChange={(e) => setConfirm(e.target.value)} aria-label={t('project.name')} placeholder={t('project.name')} style={{ maxWidth: '16rem' }} />
        <button
          type="button"
          className="btn-danger"
          disabled={confirm !== ctx.slug}
          onClick={async () => {
            await api(ctx.api(), { method: 'DELETE' })
            nav('/')
          }}
        >
          {t('common.delete')}
        </button>
      </div>
    </section>
  )
}
