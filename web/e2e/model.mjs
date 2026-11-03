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
