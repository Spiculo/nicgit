// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Lang } from '../i18n'

export interface NewsItem {
  date: string
  title: string
  body: string
}

/** Short product notes for the home page. Kept next to the code, not fetched. */
export const NEWS: Record<Lang, NewsItem[]> = {
  en: [
    {
      date: '2026-11-26',
      title: 'Home and profiles',
      body: 'Repositories, recent work and activity now share one home page. Profiles live at the account name, without a prefix.',
    },
    {
      date: '2026-11-26',
      title: 'Settings on their own pages',
      body: 'Account and project settings are split into focused pages, each with its own save.',
    },
    {
      date: '2026-11-26',
      title: 'Private by default',
      body: 'New projects stay private until you choose to publish them.',
    },
  ],
  es: [
    {
      date: '2026-11-26',
      title: 'Inicio y perfiles',
      body: 'Los repositorios, el trabajo reciente y la actividad comparten ahora una página de inicio. Los perfiles viven en el nombre de la cuenta, sin prefijo.',
    },
    {
      date: '2026-11-26',
      title: 'Configuración en páginas propias',
      body: 'La configuración de la cuenta y del proyecto se divide en páginas concretas, cada una con su propio guardado.',
    },
    {
      date: '2026-11-26',
      title: 'Privado por defecto',
      body: 'Los proyectos nuevos siguen privados hasta que decidas publicarlos.',
    },
  ],
}
