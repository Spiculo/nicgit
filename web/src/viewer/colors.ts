// SPDX-License-Identifier: AGPL-3.0-or-later
/** Redlining colours: what an architect expects on a marked-up drawing. */
export const CHANGE_COLORS = {
  added: '#2E8B57',
  modified: '#C98A12',
  removed: '#C2412D',
} as const

export type ChangeKind = keyof typeof CHANGE_COLORS

export function changeColor(kind: string): string | null {
  return (CHANGE_COLORS as Record<string, string>)[kind] ?? null
}
