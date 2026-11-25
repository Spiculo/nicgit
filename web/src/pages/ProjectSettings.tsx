// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useT } from '../i18n'
import { api, useAction, useApi } from '../lib/api'
import type { Role } from '../lib/types'
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

function Members() {
  const t = useT()
  const ctx = useProject()
  const members = useApi<{ slug: string; role: Role; via: string }[]>(ctx.api('/members'))
  const [who, setWho] = useState('')
  const [role, setRole] = useState<Role>('reviewer')
  const key = [ctx.api('/members')]
  const set = useAction((v: { who: string; role: Role }) => api(ctx.api(`/members/${v.who}`), { method: 'PUT', json: { role: v.role } }), [key])
  const remove = useAction((w: string) => api(ctx.api(`/members/${w}`), { method: 'DELETE' }), [key])
  return (
    <section id="members" className="stack">
      <h2>{t('settings.members')}</h2>
      <table className="schedule">
        <tbody>
          {members.data?.map((m) => (
            <tr key={`${m.slug}-${m.via}`}>
              <td>{m.slug}</td>
              <td>{t(`role.${m.role}`)}</td>
              <td className="muted small">{m.via}</td>
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
      <form
        className="row"
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
        <button>{t('settings.addMember')}</button>
      </form>
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
      className="stack"
      style={{ borderTop: '1px solid var(--grid)', paddingTop: '1rem' }}
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

function Rules() {
  const t = useT()
  const ctx = useProject()
  const rules = useApi<{ rules: Rule[]; settings_keys: string[] }>(ctx.api('/rules'))
  const key = [ctx.api('/rules')]
  const save = useAction((r: Rule) => api(ctx.api(`/rules/${encodeURIComponent(r.option)}`), { method: 'PUT', json: r }), [key])
  const remove = useAction((o: string) => api(ctx.api(`/rules/${encodeURIComponent(o)}`), { method: 'DELETE' }), [key])
  const [adding, setAdding] = useState(false)
  return (
    <section id="rules" className="stack">
      <h2>{t('settings.rules')}</h2>
      {rules.data?.rules.map((r) => <RuleEditor key={r.option} rule={r} onSave={(x) => save.mutate(x)} onRemove={() => remove.mutate(r.option)} />)}
      {adding ? (
        <RuleEditor
          rule={NEW_RULE}
          onSave={(x) => {
            save.mutate(x)
            setAdding(false)
          }}
        />
      ) : (
        <button type="button" onClick={() => setAdding(true)}>
          {t('settings.addRule')}
        </button>
      )}
      {save.error && <p className="error">{save.error.message}</p>}
    </section>
  )
}

function General() {
  const t = useT()
  const ctx = useProject()
  const p = ctx.overview.project
  const [description, setDescription] = useState(p.description)
  const [visibility, setVisibility] = useState(p.visibility)
  const [def, setDef] = useState(p.default_option)
  const save = useAction(() => api(ctx.api(), { method: 'PATCH', json: { description, visibility, default_option: def } }), [[ctx.api('/overview')]])
  return (
    <form
      id="general"
      className="stack"
      onSubmit={(e) => {
        e.preventDefault()
        save.mutate(undefined)
      }}
    >
      <h2>{t('settings.general')}</h2>
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
    </form>
  )
}

function Danger() {
  const t = useT()
  const ctx = useProject()
  const nav = useNavigate()
  const [confirm, setConfirm] = useState('')
  return (
    <section id="danger" className="stack">
      <h2>{t('settings.danger')}</h2>
      <p>{t('settings.deleteConfirm')}</p>
      <div className="row">
        <input value={confirm} onChange={(e) => setConfirm(e.target.value)} aria-label={t('project.name')} style={{ maxWidth: '16rem' }} />
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

function Placeholder({ id, title, body, link }: { id: string; title: string; body: string; link?: { to: string; label: string } }) {
  return (
    <section id={id} className="stack">
      <h2>{title}</h2>
      <p>{body}</p>
      {link && <Link to={link.to}>{link.label}</Link>}
    </section>
  )
}

export function ProjectSettings() {
  const t = useT()
  return (
    <main className="page stack" style={{ maxWidth: '52rem' }}>
      <h1>{t('settings.title')}</h1>
      <p className="muted">{t('settings.subtitle')}</p>
      <General />
      <Members />
      <Rules />
      <Placeholder id="ai" title={t('settings.ai')} body={t('config.aiBody')} link={{ to: '/account', label: t('config.openAccount') }} />
      <Placeholder id="budgets" title={t('config.budgets')} body={t('config.budgetsBody')} link={{ to: '/account', label: t('config.openAccount') }} />
      <Placeholder id="webhooks" title={t('config.webhooks')} body={t('config.webhooksBody')} />
      <Placeholder id="audit" title={t('config.audit')} body={t('config.auditBody')} />
      <Danger />
    </main>
  )
}
