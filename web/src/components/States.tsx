// SPDX-License-Identifier: AGPL-3.0-or-later
import { useT } from '../i18n'
import type { ApiError } from '../lib/api'

export function NotFound() {
  const t = useT()
  return (
    <div className="page">
      <p>{t('common.notFound')}</p>
    </div>
  )
}

export function Loading() {
  const t = useT()
  return <p className="muted">{t('common.loading')}</p>
}

export function ErrorNote({ error }: { error: ApiError | Error | null | undefined }) {
  const t = useT()
  if (!error) return null
  if ('status' in error && error.status === 404) return <NotFound />
  return (
    <p className="notice bad" role="alert">
      {t('common.error', { msg: error.message })}
    </p>
  )
}

export function JobWait({ progress, label }: { progress: number; label?: string }) {
  const t = useT()
  const pct = Math.round(progress * 100)
  return (
    <div className="stack" role="status">
      <p className="muted">{label ?? (pct > 0 ? t('job.progress', { p: pct }) : t('job.waiting'))}</p>
      <div className="progress" aria-hidden>
        <span style={{ width: `${Math.max(pct, 4)}%` }} />
      </div>
    </div>
  )
}

export function Change({ kind, children }: { kind: string; children?: React.ReactNode }) {
  return <span className={`change ${kind}`}>{children}</span>
}
