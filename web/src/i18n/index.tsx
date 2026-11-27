// SPDX-License-Identifier: AGPL-3.0-or-later
import { createContext, useContext } from 'react'
import { en, type Key } from './en'
import { es } from './es'
import { STYLE_TERMS, type Style } from './styles'

export type Lang = 'en' | 'es'
export type { Style }
const dictionaries: Record<Lang, Record<Key, string>> = { en, es }

export function browserLang(): Lang {
  return navigator.language.toLowerCase().startsWith('es') ? 'es' : 'en'
}

export const LangContext = createContext<Lang>('en')
export const StyleContext = createContext<Style>('git')

export type T = (key: Key, vars?: Record<string, string | number>) => string

export function translate(lang: Lang, style: Style, key: Key, vars?: Record<string, string | number>): string {
  let s = STYLE_TERMS[style][lang][key] ?? dictionaries[lang][key] ?? en[key] ?? key
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v))
  return s
}

export function useT(): T {
  const lang = useContext(LangContext)
  const style = useContext(StyleContext)
  return (key, vars) => translate(lang, style, key, vars)
}

export function useLang(): Lang {
  return useContext(LangContext)
}

export function useStyle(): Style {
  return useContext(StyleContext)
}
