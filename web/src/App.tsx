// SPDX-License-Identifier: AGPL-3.0-or-later
import { Link, Navigate, Route, Routes } from 'react-router-dom'
import { LangContext, browserLang, useT } from './i18n'
import { useMe, api } from './lib/api'
import { useQueryClient } from '@tanstack/react-query'
import { Login, Signup } from './pages/Auth'
import { Dashboard, Explore } from './pages/Dashboard'
import { NewProject } from './pages/NewProject'
import { ImportProject } from './pages/ImportProject'
import { Account } from './pages/Account'
import { ProjectLayout } from './pages/ProjectLayout'
import { NotFound } from './components/States'

export function App() {
  const me = useMe()
  const lang = me.data?.lang ?? browserLang()
  if (me.isLoading) return null
  const signedIn = !!me.data
  return (
    <LangContext.Provider value={lang}>
      <TopBar />
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/signup" element={<Signup />} />
        <Route path="/" element={signedIn ? <Dashboard /> : <Navigate to="/login" replace />} />
        <Route path="/explore" element={<Explore />} />
        <Route path="/new" element={signedIn ? <NewProject /> : <Navigate to="/login" replace />} />
        <Route path="/import" element={signedIn ? <ImportProject /> : <Navigate to="/login" replace />} />
        <Route path="/account" element={signedIn ? <Account /> : <Navigate to="/login" replace />} />
        <Route path="/:owner/:slug/*" element={<ProjectLayout />} />
        <Route path="*" element={<NotFound />} />
      </Routes>
    </LangContext.Provider>
  )
}

function TopBar() {
  const t = useT()
  const me = useMe()
  const qc = useQueryClient()
  async function signOut() {
    await api('/api/auth/logout', { method: 'POST' })
    qc.clear()
    window.location.assign('/login')
  }
  return (
    <header className="topbar">
      <Link to="/" className="wordmark">
        {t('app.name')}
      </Link>
      <div id="crumbs" className="crumbs" />
      <div className="spacer" />
      <nav aria-label="Main">
        <Link to="/explore">{t('nav.explore')}</Link>
        {me.data ? (
          <>
            <Link to="/new">{t('nav.newProject')}</Link>
            <Link to="/account">{me.data.slug}</Link>
            <button type="button" className="btn-quiet" onClick={signOut}>
              {t('nav.signOut')}
            </button>
          </>
        ) : (
          <Link to="/login">{t('nav.signIn')}</Link>
        )}
      </nav>
    </header>
  )
}
