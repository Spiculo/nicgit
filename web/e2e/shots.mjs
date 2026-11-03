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
await browser.close()
