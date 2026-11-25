// SPDX-License-Identifier: AGPL-3.0-or-later
import { useT } from '../i18n'

interface Section {
  id: string
  title: string
  body: string
}

interface SoonProps {
  title: string
  intro: string
  sections: Section[]
  note?: string
}

/** A tab that is specified but not built: no fake data, no dead controls. */
export function SoonPage({ title, intro, sections, note }: SoonProps) {
  const t = useT()
  return (
    <main className="page stack">
      <h1>{title}</h1>
      <p className="muted">{intro}</p>
      {sections.map((s) => (
        <section id={s.id} className="stack" key={s.id}>
          <h2>{s.title}</h2>
          <p>{s.body}</p>
        </section>
      ))}
      <p className="small muted">{note ?? t('soon.spec')}</p>
    </main>
  )
}

export function CIPage() {
  const t = useT()
  return (
    <SoonPage
      title={t('ci.title')}
      intro={t('ci.intro')}
      sections={[
        { id: 'runs', title: t('ci.runs'), body: t('ci.runsBody') },
        { id: 'checks', title: t('ci.checks'), body: t('ci.checksBody') },
        { id: 'schedules', title: t('ci.schedules'), body: t('soon.notBuilt') },
        { id: 'budgets', title: t('ci.budgets'), body: t('soon.notBuilt') },
      ]}
    />
  )
}

export function RendersPage() {
  const t = useT()
  return (
    <SoonPage
      title={t('renders.title')}
      intro={t('renders.intro')}
      sections={[
        { id: 'jobs', title: t('renders.jobs'), body: t('renders.jobsBody') },
        { id: 'presets', title: t('renders.presets'), body: t('renders.presetsBody') },
        { id: 'usage', title: t('renders.usage'), body: t('renders.usageBody') },
      ]}
      note={t('renders.spec')}
    />
  )
}

export function SheetsPage() {
  const t = useT()
  return (
    <SoonPage
      title={t('sheets.title')}
      intro={t('sheets.intro')}
      sections={[
        { id: 'list', title: t('sheets.list'), body: t('sheets.listBody') },
        { id: 'pdf', title: t('sheets.pdf'), body: t('sheets.pdfBody') },
        { id: 'revisions', title: t('sheets.revisions'), body: t('sheets.revisionsBody') },
      ]}
      note={t('sheets.spec')}
    />
  )
}

export function PackagesPage() {
  const t = useT()
  return (
    <SoonPage
      title={t('packages.title')}
      intro={t('packages.intro')}
      sections={[
        { id: 'objects', title: t('packages.objects'), body: t('packages.objectsBody') },
        { id: 'libraries', title: t('packages.libraries'), body: t('packages.librariesBody') },
        { id: 'rules', title: t('packages.rules'), body: t('packages.rulesBody') },
      ]}
      note={t('packages.spec')}
    />
  )
}

export function AgentsPage() {
  const t = useT()
  return (
    <SoonPage
      title={t('agents.title')}
      intro={t('agents.intro')}
      sections={[
        { id: 'sessions', title: t('agents.sessions'), body: t('agents.sessionsBody') },
        { id: 'tools', title: t('agents.tools'), body: t('agents.toolsBody') },
        { id: 'approvals', title: t('agents.approvals'), body: t('soon.notBuilt') },
      ]}
      note={t('agents.spec')}
    />
  )
}
