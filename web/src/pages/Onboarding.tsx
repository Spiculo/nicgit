// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useQueryClient } from '@tanstack/react-query'
import { translate, type Lang, type Style, type T } from '../i18n'
import type { Key } from '../i18n/en'
import { api, ApiError, useMe } from '../lib/api'
import { BrandMark } from '../components/BrandMark'

const WORKS = ['architect', 'engineer', 'contractor', 'manager', 'client', 'student', 'other'] as const
const SITES = ['none', 'little', 'regular', 'extensive'] as const
const FOCUS = ['residential', 'commercial', 'public', 'infrastructure', 'interiors', 'education', 'other'] as const
const TEAM_SIZES = ['solo', 'two_to_five', 'six_to_twenty', 'twenty_one_to_fifty', 'fifty_plus'] as const
const COMPANY_SIZES = ['one_to_ten', 'eleven_to_fifty', 'fifty_one_to_two_hundred', 'two_hundred_one_to_thousand', 'thousand_plus'] as const

export function Onboarding() {
  const me = useMe()
  const nav = useNavigate()
  const qc = useQueryClient()
  const [step, setStep] = useState(1)
  const [lang, setLang] = useState<Lang>(me.data?.lang === 'es' ? 'es' : 'en')
  const [style, setStyle] = useState<Style>(me.data?.style === 'architecture' ? 'architecture' : 'git')
  const [work, setWork] = useState(me.data?.work ?? '')
  const [years, setYears] = useState(me.data?.years_experience?.toString() ?? '')
  const [site, setSite] = useState(me.data?.site_experience ?? '')
  const [focus, setFocus] = useState(me.data?.focus ?? '')
  const [company, setCompany] = useState(me.data?.company ?? '')
  const [teamSize, setTeamSize] = useState(me.data?.team_size ?? '')
  const [companySize, setCompanySize] = useState(me.data?.company_size ?? '')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const t: T = (key, vars) => translate(lang, style, key, vars)
  const canContinue = work !== '' && years !== '' && site !== ''

  async function finish() {
    setBusy(true)
    setError(null)
    try {
      await api('/api/me/onboarding', {
        method: 'POST',
        json: {
          lang,
          style,
          work,
          years_experience: Number(years),
          site_experience: site,
          focus,
          company,
          team_size: teamSize,
          company_size: companySize,
        },
      })
      await qc.invalidateQueries({ queryKey: ['/api/me'] })
      nav('/', { replace: true })
    } catch (e) {
      setError(e instanceof ApiError ? e.message : String(e))
      setBusy(false)
    }
  }

  async function signOut() {
    await api('/api/auth/logout', { method: 'POST' })
    qc.clear()
    window.location.assign('/login')
  }

  if (!me.data) return null

  return (
    <main className="onboarding">
      <div className="onboarding-card">
        <header className="onboarding-head">
          <span className="onboarding-brand">
            <BrandMark size={24} />
            <strong>NicGit</strong>
          </span>
          <span className="small muted onboarding-who">{t('onboarding.signedInAs', { who: me.data.display_name || me.data.slug })}</span>
        </header>

        <div className="onboarding-progress" aria-hidden="true">
          {[1, 2, 3].map((n) => (
            <span key={n} className={n <= step ? 'on' : undefined} />
          ))}
        </div>
        <p className="onboarding-step">{t('onboarding.step', { n: step })}</p>

        {step === 1 && (
          <section>
            <h1>{t('onboarding.welcomeTitle')}</h1>
            <p className="muted">{t('onboarding.welcomeBody')}</p>
            <h2>{t('onboarding.languageTitle')}</h2>
            <div className="onboarding-choices">
              {(['en', 'es'] as const).map((l) => (
                <button key={l} type="button" className="onboarding-choice" aria-pressed={lang === l} onClick={() => setLang(l)}>
                  <strong>{l === 'en' ? 'English' : 'Español'}</strong>
                </button>
              ))}
            </div>
            <h2>{t('onboarding.wordingTitle')}</h2>
            <div className="onboarding-choices">
              {(['git', 'architecture'] as const).map((s) => (
                <button key={s} type="button" className="onboarding-choice" aria-pressed={style === s} onClick={() => setStyle(s)}>
                  <strong>{s === 'git' ? t('onboarding.wordingGit') : t('onboarding.wordingArchitecture')}</strong>
                  <span>{s === 'git' ? t('onboarding.wordingGitHint') : t('onboarding.wordingArchitectureHint')}</span>
                </button>
              ))}
            </div>
          </section>
        )}

        {step === 2 && (
          <section>
            <h1>{t('onboarding.workTitle')}</h1>
            <div className="field">
              <label htmlFor="ob-work">{t('onboarding.field')}</label>
              <select id="ob-work" value={work} onChange={(e) => setWork(e.target.value)}>
                <option value="">—</option>
                {WORKS.map((w) => (
                  <option key={w} value={w}>{t(`work.${w}` as Key)}</option>
                ))}
              </select>
            </div>
            <div className="row onboarding-fields">
              <div className="field">
                <label htmlFor="ob-years">{t('onboarding.years')}</label>
                <input id="ob-years" type="number" min={0} max={80} value={years} onChange={(e) => setYears(e.target.value)} />
              </div>
              <div className="field">
                <label htmlFor="ob-site">{t('onboarding.site')}</label>
                <select id="ob-site" value={site} onChange={(e) => setSite(e.target.value)}>
                  <option value="">—</option>
                  {SITES.map((s) => (
                    <option key={s} value={s}>{t(`site.${s}` as Key)}</option>
                  ))}
                </select>
              </div>
            </div>
            <div className="field">
              <label htmlFor="ob-focus">
                {t('onboarding.focus')} <span className="muted small">({t('onboarding.optional')})</span>
              </label>
              <select id="ob-focus" value={focus} onChange={(e) => setFocus(e.target.value)}>
                <option value="">—</option>
                {FOCUS.map((f) => (
                  <option key={f} value={f}>{t(`focus.${f}` as Key)}</option>
                ))}
              </select>
            </div>
          </section>
        )}

        {step === 3 && (
          <section>
            <h1>{t('onboarding.teamTitle')}</h1>
            <div className="field">
              <label htmlFor="ob-company">
                {t('onboarding.company')} <span className="muted small">({t('onboarding.optional')})</span>
              </label>
              <input id="ob-company" value={company} onChange={(e) => setCompany(e.target.value)} maxLength={120} />
            </div>
            <div className="row onboarding-fields">
              <div className="field">
                <label htmlFor="ob-team">{t('onboarding.teamSize')}</label>
                <select id="ob-team" value={teamSize} onChange={(e) => setTeamSize(e.target.value)}>
                  <option value="">—</option>
                  {TEAM_SIZES.map((s) => (
                    <option key={s} value={s}>{t(`teamSize.${s}` as Key)}</option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label htmlFor="ob-company-size">{t('onboarding.companySize')}</label>
                <select id="ob-company-size" value={companySize} onChange={(e) => setCompanySize(e.target.value)}>
                  <option value="">—</option>
                  {COMPANY_SIZES.map((s) => (
                    <option key={s} value={s}>{t(`companySize.${s}` as Key)}</option>
                  ))}
                </select>
              </div>
            </div>
          </section>
        )}

        {error && <p className="notice bad" role="alert">{error}</p>}

        <div className="onboarding-actions">
          {step > 1 ? (
            <button type="button" className="btn" onClick={() => setStep(step - 1)} disabled={busy}>
              {t('onboarding.back')}
            </button>
          ) : me.data.onboarded ? (
            <button type="button" className="btn" onClick={() => nav('/')} disabled={busy}>
              {t('onboarding.cancel')}
            </button>
          ) : null}
          <span className="spacer" />
          {step < 3 ? (
            <button type="button" className="btn btn-primary" disabled={step === 2 && !canContinue} onClick={() => setStep(step + 1)}>
              {t('onboarding.continue')}
            </button>
          ) : (
            <button type="button" className="btn btn-primary" disabled={busy} onClick={finish}>
              {busy ? t('onboarding.saving') : t('onboarding.finish')}
            </button>
          )}
        </div>
        <p className="onboarding-signout">
          <button type="button" className="btn-quiet" onClick={signOut}>
            {t('onboarding.signOut')}
          </button>
        </p>
      </div>
    </main>
  )
}
