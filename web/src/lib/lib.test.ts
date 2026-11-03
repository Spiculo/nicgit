// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, test } from 'vitest'
import { en } from '../i18n/en'
import { es } from '../i18n/es'
import { changeColor, CHANGE_COLORS } from '../viewer/colors'
import { optionLabel, optionPath, shortId, plural } from './format'

describe('i18n', () => {
  test('Spanish has exactly the English keys', () => {
    expect(Object.keys(es).sort()).toEqual(Object.keys(en).sort())
  })
  test('placeholders match between languages', () => {
    const holes = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join(',')
    for (const k of Object.keys(en) as (keyof typeof en)[]) {
      expect(holes(es[k]), k).toBe(holes(en[k]))
    }
  })
})

describe('change colours', () => {
  test('each kind has its own colour and unknown kinds are neutral', () => {
    expect(new Set(Object.values(CHANGE_COLORS)).size).toBe(3)
    expect(changeColor('added')).toBe(CHANGE_COLORS.added)
    expect(changeColor('removed')).toBe(CHANGE_COLORS.removed)
    expect(changeColor('whatever')).toBeNull()
  })
})

describe('format', () => {
  test('option labels decode the app’s percent-encoding', () => {
    expect(optionLabel('Scheme%20B')).toBe('Scheme B')
    expect(optionLabel('agent/widen-windows')).toBe('agent/widen-windows')
    expect(optionLabel('bad%zz')).toBe('bad%zz')
  })
  test('option names are safe in a URL path segment', () => {
    expect(optionPath('Scheme%20B')).toBe('Scheme%2520B')
    expect(optionPath('agent/x')).toBe('agent%2Fx')
  })
  test('short ids and plurals', () => {
    expect(shortId('b8c34ff0123456789')).toBe('b8c34ff')
    expect(plural(1, 'wall', 'walls')).toBe('1 wall')
    expect(plural(3, 'wall', 'walls')).toBe('3 walls')
  })
})
