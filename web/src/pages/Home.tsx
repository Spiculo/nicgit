// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { useLang, useT } from '../i18n'
import type { T } from '../i18n'
import { useApi } from '../lib/api'
import { whenIso } from '../lib/format'
import { CHANGELOGS, type ChangelogProduct } from '../lib/changelog'
import { NEWS } from '../lib/news'
import type { HomeData, HomePost, HomeRepo, HomeRun } from '../lib/types'
import { ErrorNote, Loading } from '../components/States'
import { Icon } from '../components/Icon'

function Panel({ title, empty, children }: { title: string; empty?: string; children?: ReactNode }) {
  return (
    <section className="home-panel">
      <h2>{title}</h2>
      {children ?? (empty && <p className="home-empty">{empty}</p>)}
    </section>
  )
}

function RepoItem({ repo }: { repo: HomeRepo }) {
  const t = useT()
  const lang = useLang()
  const proposals = repo.open_proposals === 1 ? t('home.proposalOne') : t('home.proposalsCount', { n: repo.open_proposals })
  const issues = repo.open_issues === 1 ? t('home.issueOne') : t('home.issuesCount', { n: repo.open_issues })
  return (
    <li className="home-repo">
      <div className="home-repo-top">
        <Link className="home-repo-name" to={`/${repo.full_name}`}>
          {repo.full_name}
        </Link>
        <span className="tag">{repo.visibility === 'public' ? t('dash.public') : t('dash.private')}</span>
      </div>
      {repo.description && <p className="small muted">{repo.description}</p>}
      <div className="home-repo-meta">
        <span className="small muted">{whenIso(repo.last_activity_at, lang)}</span>
        {!!repo.open_proposals && <span className="small muted">{proposals}</span>}
        {!!repo.open_issues && <span className="small muted">{issues}</span>}
      </div>
    </li>
  )
}

function PostLine({ post }: { post: HomePost }) {
  const t = useT()
  const d = post.data
  const project = String(d.project ?? '')
  const number = String(d.number ?? '')
  const title = String(d.title ?? '')
  switch (post.kind) {
    case 'project.added':
      return <Link to={`/${project}`}>{t('post.added', { project })}</Link>
    case 'proposal.review_requested':
      return <Link to={`/${project}/proposals/${number}`}>{t('post.reviewRequested', { project, number, title })}</Link>
    case 'proposal.reviewed':
      return <Link to={`/${project}/proposals/${number}`}>{t('post.reviewed', { project, number, title })}</Link>
    case 'proposal.combined':
      return <Link to={`/${project}/proposals/${number}`}>{t('post.combined', { project, number, title })}</Link>
    case 'issue.assigned':
      return <Link to={`/${project}/issues/${number}`}>{t('post.assigned', { project, number, title })}</Link>
    default:
      return <Link to={`/${project}`}>{project}</Link>
  }
}

function runLabel(run: HomeRun, t: T): string {
  if (run.status === 'done') return run.result?.passed ? t('run.passed') : t('run.done')
  if (run.status === 'failed') return t('run.failed')
  if (run.status === 'running') return t('run.running')
  return t('run.queued')
}

function runTone(run: HomeRun): string {
  if (run.status === 'failed' || (run.status === 'done' && !run.result?.passed)) return 'bad'
  if (run.status === 'done') return 'ok'
  return 'warn'
}


export function Home() {
  const t = useT()
  const lang = useLang()
  const home = useApi<HomeData>('/api/me/home')
  const [query, setQuery] = useState('')
  const [log, setLog] = useState<ChangelogProduct>('nicgit')
  if (home.isLoading) return <main className="home"><div className="page"><Loading /></div></main>
  if (home.error) return <main className="home"><div className="page"><ErrorNote error={home.error} /></div></main>
  if (!home.data) return null
  const d = home.data
  const q = query.trim().toLowerCase()
  const repos = d.repos.filter((r) => !q || `${r.full_name} ${r.description}`.toLowerCase().includes(q))

  return (
    <main className="home">
      <header className="home-title">
        <h1>{t('home.title')}</h1>
      </header>
      <div className="home-grid">
        <aside className="home-left">
          <div className="home-section-head">
            <h2>{t('home.repositories')}</h2>
            <Link className="btn btn-small" to="/new">
              <Icon name="plus" size={13} />
              {t('home.new')}
            </Link>
          </div>
          {d.repos.length > 4 && (
            <input
              type="search"
              className="home-filter"
              aria-label={t('home.filterRepos')}
              placeholder={t('home.filterRepos')}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          )}
          {repos.length ? (
            <ul className="home-repos">
              {repos.map((repo) => (
                <RepoItem key={repo.id} repo={repo} />
              ))}
            </ul>
          ) : (
            <p className="home-empty">{d.repos.length ? t('home.noMatches') : t('home.noRepos')}</p>
          )}
        </aside>

        <section className="home-center">
          <Panel title={t('home.posts')} empty={t('home.noPosts')}>
            {d.posts.length ? (
              <ul className="home-posts">
                {d.posts.map((post) => (
                  <li key={post.id} className={`home-post${post.read ? '' : ' unread'}`}>
                    <PostLine post={post} />
                    <div className="small muted">{whenIso(post.created_at, lang)}</div>
                  </li>
                ))}
              </ul>
            ) : null}
          </Panel>

          <div className="home-panels">
            <Panel title={t('home.releases')} empty={t('home.noReleases')}>
              {d.releases.length ? (
                <ul className="home-rows">
                  {d.releases.map((r) => (
                    <li key={`${r.project}-${r.tag}`} className="home-row">
                      <div>
                        <Link to={`/${r.project}/milestones`}>
                          <strong>{r.tag}</strong>
                        </Link>{' '}
                        <span className="small muted">{r.title}</span>
                      </div>
                      <div className="home-row-meta small muted">
                        {r.project} · {whenIso(r.created_at, lang)}
                      </div>
                    </li>
                  ))}
                </ul>
              ) : null}
            </Panel>

            <Panel title={t('home.packages')} empty={t('home.noPackages')} />

            <Panel title={t('home.pullRequests')} empty={t('home.noPullRequests')}>
              {d.proposals.length ? (
                <ul className="home-rows">
                  {d.proposals.map((p) => (
                    <li key={`${p.project}-${p.number}`} className="home-row">
                      <Link to={`/${p.project}/proposals/${p.number}`}>
                        #{p.number} {p.title}
                      </Link>
                      <div className="home-row-meta small muted">
                        {p.project} · {p.author} · {whenIso(p.created_at, lang)}
                      </div>
                    </li>
                  ))}
                </ul>
              ) : null}
            </Panel>

            <Panel title={t('home.issues')} empty={t('home.noIssues')}>
              {d.issues.length ? (
                <ul className="home-rows">
                  {d.issues.map((i) => (
                    <li key={`${i.project}-${i.number}`} className="home-row">
                      <Link to={`/${i.project}/issues/${i.number}`}>
                        #{i.number} {i.title}
                      </Link>
                      <div className="home-row-meta small muted">
                        <span className="tag">{i.priority}</span> {i.project} · {whenIso(i.created_at, lang)}
                      </div>
                    </li>
                  ))}
                </ul>
              ) : null}
            </Panel>

            <Panel title={t('home.runs')} empty={t('home.noRuns')}>
              {d.runs.length ? (
                <ul className="home-rows">
                  {d.runs.map((r) => (
                    <li key={`${r.project}-${r.id}`} className="home-row">
                      <div>
                        <span className={`tag ${runTone(r)}`}>{runLabel(r, t)}</span>{' '}
                        <Link to={`/${r.project}/ci`}>{r.project}</Link>
                      </div>
                      <div className="home-row-meta small muted">
                        {whenIso(r.finished_at ?? r.created_at, lang)}
                      </div>
                    </li>
                  ))}
                </ul>
              ) : null}
            </Panel>
          </div>
        </section>

        <aside className="home-right">
          <Panel title={t('home.changeLog')}>
            <div className="home-log-tabs" role="tablist" aria-label={t('home.changeLog')}>
              {(['nicgit', 'nicbim'] as const).map((p) => (
                <button key={p} type="button" role="tab" aria-selected={log === p} onClick={() => setLog(p)}>
                  {p === 'nicgit' ? 'NicGit' : 'NicBim'}
                </button>
              ))}
            </div>
            <ul className="home-rows">
              {CHANGELOGS[lang][log].map((e) => (
                <li key={`${e.date}-${e.title}`} className="home-row">
                  <strong>{e.title}</strong>
                  <p className="small muted">{e.body}</p>
                  <span className="small muted">{e.date}</span>
                </li>
              ))}
            </ul>
          </Panel>

          <Panel title={t('home.news')}>
            <ul className="home-news">
              {NEWS[lang].map((n) => (
                <li key={n.title}>
                  <strong>{n.title}</strong>
                  <p className="small muted">{n.body}</p>
                  <span className="small muted">{n.date}</span>
                </li>
              ))}
            </ul>
          </Panel>
        </aside>
      </div>
    </main>
  )
}
