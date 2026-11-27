// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Lang } from '../i18n'

export type ChangelogProduct = 'nicgit' | 'nicbim'

export interface ChangelogEntry {
  date: string
  title: string
  body: string
}

/**
 * Recent entries for the two products. NicGit follows CHANGELOG.md; NicBim follows its decision log
 * until it publishes its own changelog. Keep this list short: the home panel is a summary, not the
 * full history.
 */
export const CHANGELOGS: Record<Lang, Record<ChangelogProduct, ChangelogEntry[]>> = {
  en: {
    nicgit: [
      { date: '2026-11-26', title: 'Home and profiles', body: 'Repositories by activity, recent work, news, and profiles at the account name.' },
      { date: '2026-11-25', title: 'GitHub-shaped navigation', body: 'Repository tabs, centred pages, and settings split into pages.' },
      { date: '2026-11-25', title: 'Branches, releases and organisations', body: 'Branch management, milestones and tags, teams and grants.' },
      { date: '2026-11-25', title: 'NicBim .nic engine', body: 'Import and export .nic with the pinned engine.' },
    ],
    nicbim: [
      { date: '2026-11-25', title: 'macOS (Apple Silicon)', body: 'The app, Rust extension and sample house run on macOS 15+.' },
      { date: '2026-11-19', title: '.nic project files', body: '.kim becomes .nic; parametric objects use .nicobj.' },
      { date: '2026-11-13', title: 'Engineering disciplines', body: 'MEP, HVAC, plumbing, electrical and structural checks.' },
      { date: '2026-11-13', title: 'Structural analysis', body: 'Loads, supports, frame analysis and member checks.' },
    ],
  },
  es: {
    nicgit: [
      { date: '2026-11-26', title: 'Inicio y perfiles', body: 'Repositorios por actividad, trabajo reciente, noticias y perfiles en el nombre de la cuenta.' },
      { date: '2026-11-25', title: 'Navegación con forma de GitHub', body: 'Pestañas de repositorio, páginas centradas y configuración dividida en páginas.' },
      { date: '2026-11-25', title: 'Ramas, publicaciones y organizaciones', body: 'Gestión de ramas, hitos y etiquetas, equipos y accesos.' },
      { date: '2026-11-25', title: 'Motor .nic de NicBim', body: 'Importa y exporta .nic con el motor fijado.' },
    ],
    nicbim: [
      { date: '2026-11-25', title: 'macOS (Apple Silicon)', body: 'La app, la extensión Rust y la casa de ejemplo funcionan en macOS 15+.' },
      { date: '2026-11-19', title: 'Archivos de proyecto .nic', body: '.kim pasa a .nic; los objetos paramétricos usan .nicobj.' },
      { date: '2026-11-13', title: 'Disciplinas de ingeniería', body: 'MEP, HVAC, fontanería, electricidad y comprobaciones estructurales.' },
      { date: '2026-11-13', title: 'Análisis estructural', body: 'Cargas, apoyos, análisis de pórticos y comprobaciones de barras.' },
    ],
  },
}
