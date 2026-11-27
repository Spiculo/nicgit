// SPDX-License-Identifier: AGPL-3.0-or-later
import { useEffect, type ReactNode } from 'react'
import { Link, Navigate, Route, Routes, useLocation, useParams } from 'react-router-dom'
import { LangContext, StyleContext, browserLang, useT } from './i18n'
import { useMe, api } from './lib/api'
import { useQueryClient } from '@tanstack/react-query'
import { Login } from './pages/Auth'
import { Onboarding } from './pages/Onboarding'
import { Home } from './pages/Home'
import { Explore } from './pages/Explore'
import { Profile } from './pages/Profile'
import { NewProject } from './pages/NewProject'
import { ImportProject } from './pages/ImportProject'
import { ProjectLayout } from './pages/ProjectLayout'
import {
  UserSettingsLayout,
  ProfileSettings,
  AccessSettings,
  AppearanceSettings,
  OrganizationsSettings,
  AiSettings,
  BillingSettings,
} from './pages/UserSettings'
import { NotFound } from './components/States'
import { BrandMark } from './components/BrandMark'
import { Icon } from './components/Icon'

export function App() {
  const me = useMe()
  const { pathname } = useLocation()
  const lang = me.data?.lang ?? browserLang()
  const style = me.data?.style === 'architecture' ? 'architecture' : 'git'
  if (me.isLoading) return null
  const signedIn = !!me.data
  if (me.data && !me.data.onboarded && pathname !== '/welcome') {
    return <Navigate to="/welcome" replace />
  }
  const welcome = pathname === '/welcome'
  return (
    <LangContext.Provider value={lang}>
      <StyleContext.Provider value={style}>
        {!welcome && <TopBar />}
        <Routes>
          <Route path="/welcome" element={<Guarded signedIn={signedIn}><Onboarding /></Guarded>} />
          <Route path="/login" element={<Login />} />
          <Route path="/signup" element={<Navigate to="/login" replace />} />
          <Route path="/" element={<Guarded signedIn={signedIn}><Home /></Guarded>} />
          <Route path="/explore" element={<Explore />} />
          <Route path="/new" element={<Guarded signedIn={signedIn}><NewProject /></Guarded>} />
          <Route path="/import" element={<Guarded signedIn={signedIn}><ImportProject /></Guarded>} />
          <Route path="/settings" element={<Guarded signedIn={signedIn}><UserSettingsLayout /></Guarded>}>
            <Route index element={<Navigate to="profile" replace />} />
            <Route path="profile" element={<ProfileSettings />} />
            <Route path="access" element={<AccessSettings />} />
            <Route path="appearance" element={<AppearanceSettings />} />
            <Route path="organizations" element={<OrganizationsSettings />} />
            <Route path="ai" element={<AiSettings />} />
            <Route path="billing" element={<BillingSettings />} />
          </Route>
          <Route path="/account" element={<Navigate to="/settings/profile" replace />} />
          <Route path="/orgs/:slug" element={<OrgRedirect />} />
          <Route path="/:slug" element={<Profile />} />
          <Route path="/:owner/:slug/*" element={<ProjectLayout />} />
          <Route path="*" element={<NotFound />} />
        </Routes>
      </StyleContext.Provider>
    </LangContext.Provider>
  )
}

function Guarded({ signedIn, children }: { signedIn: boolean; children: ReactNode }) {
  return signedIn ? <>{children}</> : <Navigate to="/login" replace />
}

function OrgRedirect() {
  const { slug = '' } = useParams()
  return <Navigate to={`/${slug}`} replace />
}

function TopBar() {
  const t = useT()
  const me = useMe()
  const qc = useQueryClient()
  const initial = (me.data?.display_name?.trim() || me.data?.slug || '?').slice(0, 1)
  useEffect(() => {
    const closeMenus = (e: MouseEvent) => {
      for (const d of Array.from(document.querySelectorAll<HTMLDetailsElement>('details.menu-wrap[open]'))) {
        if (!d.contains(e.target as Node)) d.open = false
      }
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      for (const d of Array.from(document.querySelectorAll<HTMLDetailsElement>('details.menu-wrap[open]'))) d.open = false
    }
    document.addEventListener('click', closeMenus)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('click', closeMenus)
      document.removeEventListener('keydown', onKey)
    }
  }, [])
  async function signOut() {
    await api('/api/auth/logout', { method: 'POST' })
    qc.clear()
    window.location.assign('/login')
  }
  const closeMenu = (e: React.MouseEvent<HTMLElement>) => {
    const details = e.currentTarget.closest('details') as HTMLDetailsElement | null
    if (details) details.open = false
  }
  return (
    <header className="topbar">
      <div className="topbar-inner">
        <Link to={me.data ? '/' : '/explore'} className="brand" aria-label={t('app.name')}>
          <BrandMark />
          <span>{t('app.name')}</span>
        </Link>
        <nav className="topnav" aria-label={t('nav.main')}>
          <Link to="/explore" aria-label={t('nav.explore')}>
            <Icon name="globe" />
            <span>{t('nav.explore')}</span>
          </Link>
        </nav>
        <span className="spacer" />
        <nav className="topnav" aria-label={t('nav.account')}>
          {me.data ? (
            <>
              <details className="menu-wrap">
                <summary aria-label={t('nav.new')}>
                  <Icon name="plus" />
                  <span className="label">{t('nav.new')}</span>
                  <Icon name="chevron" size={13} className="chev" />
                </summary>
                <div className="menu">
                  <Link to="/new" onClick={closeMenu}>
                    <Icon name="plus" />
                    {t('nav.newProject')}
                  </Link>
                  <Link to="/import" onClick={closeMenu}>
                    <Icon name="file" />
                    {t('nav.importShort')}
                  </Link>
                </div>
              </details>
              <details className="menu-wrap">
                <summary className="avatar-btn" aria-label={me.data.slug}>
                  <span className="avatar">{initial}</span>
                  <Icon name="chevron" size={13} className="chev" />
                </summary>
                <div className="menu">
                  <div className="who">
                    <strong>{me.data.display_name}</strong>
                    <div className="small muted">{me.data.email}</div>
                  </div>
                  <div className="divider" />
                  <Link to={`/${me.data.slug}`} onClick={closeMenu}>
                    <Icon name="user" />
                    {t('nav.yourProfile')}
                  </Link>
                  <Link to="/settings/profile" onClick={closeMenu}>
                    <Icon name="gear" />
                    {t('nav.settings')}
                  </Link>
                  <div className="divider" />
                  <button type="button" onClick={signOut}>
                    <Icon name="signout" />
                    {t('nav.signOut')}
                  </button>
                </div>
              </details>
            </>
          ) : (
            <Link to="/login">{t('nav.signIn')}</Link>
          )}
        </nav>
      </div>
    </header>
  )
}
