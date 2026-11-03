// SPDX-License-Identifier: AGPL-3.0-or-later
import { createContext, useContext } from 'react'
import { en, type Key } from './en'
import { es } from './es'

export type Lang = 'en' | 'es'
const dictionaries: Record<Lang, Record<Key, string>> = { en, es }

export function browserLang(): Lang {
  return navigator.language.toLowerCase().startsWith('es') ? 'es' : 'en'
}

export const LangContext = createContext<Lang>('en')

export type T = (key: Key, vars?: Record<string, string | number>) => string

export function translate(lang: Lang, key: Key, vars?: Record<string, string | number>): string {
  let s = dictionaries[lang][key] ?? en[key] ?? key
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v))
  return s
}

export function useT(): T {
  const lang = useContext(LangContext)
  return (key, vars) => translate(lang, key, vars)
}

export function useLang(): Lang {
  return useContext(LangContext)
}
