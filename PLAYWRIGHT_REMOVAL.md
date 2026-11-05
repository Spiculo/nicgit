# Playwright removed — 2026-11-04

Playwright was removed from this repository and from the development machine on **2026-11-04**.

## Why

- Playwright bundles its own Chromium. This repo pinned `@playwright/test@1.49`, which ships
  Chromium 131 — older than 156 and missing every fix below. The machine-wide install was
  Playwright 1.61.0, whose cache included revision **1248** (`chromium-1248`) reporting
  **156.0.8078.4**. Both are below the fixed build.
- Four critical Chrome 156 issues are fixed only in **156.0.8078.73**:

  | CVE | Component | Weakness | Impact |
  | --- | --- | --- | --- |
  | CVE-2026-118306 | FileSystem | CWE-119 memory corruption | Remote code execution **outside the browser sandbox** (CVSS 9.6, Chromium Critical) |
  | CVE-2026-118331 | FileAPI | CWE-20 improper input validation | Arbitrary code execution via a crafted page |
  | CVE-2026-118348 | Skia | CWE-20 improper input validation | Arbitrary code execution via a crafted page |
  | CVE-2026-118344 | Layout | CWE-416 use-after-free | Arbitrary code execution via a crafted page |

  Fixed in Chrome 156.0.8078.73 (Windows/Mac 156.0.8078.73/.74). References: NVD, the Chrome
  156.0.8078.73 security release, Debian DSA-6592-1.
- Playwright's `chromiumSandbox` launch option **defaults to `false`**, so Playwright runs without
  the Chromium sandbox even on a patched browser build.
- Net effect: any script that opens untrusted content (redirects, iframes, ads, a compromised
  dependency) executes with the privileges of the logged-in user.

## What was removed from this repo

- `web/package.json`: `@playwright/test@1.49` devDependency (this was an uncommitted change).
- `web/package-lock.json`: Playwright entries (removed via `npm uninstall @playwright/test`).
- `web/node_modules/@playwright`.
- `web/e2e/shots.mjs` and `web/e2e/model.mjs` (screenshot helpers). Sources archived below.
- The now-empty `web/e2e/` directory.

`web` has no `test:e2e` script; this dependency was not wired into `npm test`, which still runs
`vitest run`. Nothing else in the build referenced Playwright.

Historical planning notes under `docs/superpowers/` still mention Playwright; they were left as
history and are not build inputs.

## What still covers `web`

- `npm test` in `web/` — Vitest. Unchanged.
- `npm run build` — typecheck + Vite build. Unchanged.

## If you need screenshots or browser automation again

Do **not** reinstall Playwright with its bundled browser. In order of preference:

1. **WebdriverIO or Selenium against system Chrome/Edge**, OS sandbox enabled, Chrome
   **>= 156.0.8078.73**.
2. **Playwright only in a disposable VM/container** with `chromiumSandbox: true`, non-root user, no
   host credentials mounted, browser >= 156.0.8078.73.
3. For screenshots specifically, use the system browser's headless mode
   (`chrome --headless --screenshot`) or a plain CDP script against system Chrome.

Check the browser build before trusting any runner: `chrome://version`, or
`chrome.exe --version` should report >= 156.0.8078.73.

---

## Appendix: removed `web/e2e/shots.mjs`

```js
// SPDX-License-Identifier: AGPL-3.0-or-later
// Screenshots of the main screens against a running dev server with demo data (scripts/seed-demo.sh).
import { chromium } from '@playwright/test'
const base = process.env.NICGIT_URL ?? 'http://127.0.0.1:3000'
const out = process.argv[2] ?? 'shots'
const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader'] })
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
page.on('console', (m) => m.type() === 'error' && console.log('console:', m.text()))
page.on('pageerror', (e) => console.log('pageerror:', e.message))
await page.goto(`${base}/login`)
await page.screenshot({ path: `${out}/01-login.png` })
await page.fill('#login', 'ana')
await page.fill('#password', 'correct horse battery')
await page.click('button.btn-primary')
await page.waitForURL(`${base}/`)
await page.screenshot({ path: `${out}/02-dashboard.png` })
async function settle() {
  await page.waitForSelector('.viewer', { timeout: 30000 })
  await page.waitForTimeout(800)
  await page.waitForFunction(() => !document.querySelector('.viewer-status'), null, { timeout: 180000 }).catch(() => console.log('viewer still busy'))
  await page.waitForTimeout(500)
}
await page.goto(`${base}/studio/sample-house`)
await settle()
await page.screenshot({ path: `${out}/03-model.png` })
const canvas = page.locator('.viewer-canvas canvas')
const box = await canvas.boundingBox()
await page.mouse.click(box.x + box.width * 0.48, box.y + box.height * 0.52)
await page.waitForTimeout(1500)
await page.screenshot({ path: `${out}/04-selected.png` })
await page.goto(`${base}/studio/sample-house/versions`)
await page.waitForTimeout(1500)
await page.screenshot({ path: `${out}/05-versions.png` })
await page.goto(`${base}/studio/sample-house/compare?from=main~1&to=Scheme%2520B`)
await page.waitForSelector('.side', { timeout: 180000 })
await settle()
await page.screenshot({ path: `${out}/06-compare.png` })
await page.goto(`${base}/studio/sample-house/proposals/1`)
await page.waitForTimeout(1500)
await page.screenshot({ path: `${out}/07-proposal.png` })
await page.click('role=tab[name="Conflicts"]')
await page.waitForSelector('text=How to settle conflicts', { timeout: 120000 }).catch(() => console.log('no conflicts section'))
await page.screenshot({ path: `${out}/08-conflicts.png`, fullPage: true })
await page.click('text=Take Scheme B everywhere they conflict')
await page.click('button:has-text("Combine")')
await page.waitForSelector('text=Combined into main', { timeout: 300000 })
await page.click('role=tab[name="Overview"]')
await page.screenshot({ path: `${out}/09-combined.png` })
await browser.close()
```

## Appendix: removed `web/e2e/model.mjs`

```js
// SPDX-License-Identifier: AGPL-3.0-or-later
// Screenshot one project's model view: node e2e/model.mjs <owner/project> <out.png>
import { chromium } from '@playwright/test'
const base = process.env.NICGIT_URL ?? 'http://127.0.0.1:3000'
const [project, out] = process.argv.slice(2)
const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader'] })
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
page.on('pageerror', (e) => console.log('pageerror:', e.message))
await page.goto(`${base}/login`)
await page.fill('#login', 'ana')
await page.fill('#password', 'correct horse battery')
await page.click('button.btn-primary')
await page.waitForURL(`${base}/`)
const t0 = Date.now()
await page.goto(`${base}/${project}`)
await page.waitForSelector('.viewer')
await page.waitForTimeout(800)
await page.waitForFunction(() => !document.querySelector('.viewer-status'), null, { timeout: 600000 })
console.log(`model shown after ${((Date.now() - t0) / 1000).toFixed(1)} s`)
await page.waitForTimeout(500)
await page.screenshot({ path: out })
await browser.close()
```
