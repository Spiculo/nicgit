// SPDX-License-Identifier: AGPL-3.0-or-later
import { useT } from '../i18n'

function EmptyTab({ title, empty }: { title: string; empty: string }) {
  return (
    <main className="page">
      <header className="page-head">
        <h1>{title}</h1>
      </header>
      <div className="empty">
        <p>{empty}</p>
      </div>
    </main>
  )
}

export function RendersPage() {
  const t = useT()
  return <EmptyTab title={t('renders.title')} empty={t('renders.empty')} />
}

export function SheetsPage() {
  const t = useT()
  return <EmptyTab title={t('sheets.title')} empty={t('sheets.empty')} />
}

export function PackagesPage() {
  const t = useT()
  return <EmptyTab title={t('packages.title')} empty={t('packages.empty')} />
}

export function AgentsPage() {
  const t = useT()
  return <EmptyTab title={t('agents.title')} empty={t('agents.empty')} />
}
