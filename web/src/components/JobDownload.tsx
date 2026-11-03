// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react'
import { useT } from '../i18n'

/** A download the engine prepares first (a .kim export): waits for the job, then starts the download. */
export function JobDownload({ href, children }: { href: string; children: React.ReactNode }) {
  const t = useT()
  const [state, setState] = useState<'idle' | 'working' | 'failed'>('idle')
  const [msg, setMsg] = useState('')
  async function go() {
    setState('working')
    for (;;) {
      const ctrl = new AbortController()
      const r = await fetch(href, { credentials: 'same-origin', signal: ctrl.signal })
      if (r.status === 202) {
        await new Promise((res) => setTimeout(res, 1000))
        continue
      }
      ctrl.abort()
      if (r.ok) {
        window.location.assign(href)
        setState('idle')
      } else {
        setMsg(await r.text().catch(() => ''))
        setState('failed')
      }
      return
    }
  }
  return (
    <>
      <button type="button" onClick={go} disabled={state === 'working'}>
        {state === 'working' ? t('job.waiting') : children}
      </button>
      {state === 'failed' && <span className="error small">{t('job.failed', { msg })}</span>}
    </>
  )
}
