// SPDX-License-Identifier: AGPL-3.0-or-later

/** The desktop app percent-encodes design option names in git refs ("Scheme%20B"). */
export function optionLabel(name: string): string {
  try {
    return decodeURIComponent(name)
  } catch {
    return name
  }
}

/** An option name as one URL path segment (slashes and percent signs escaped). */
export function optionPath(name: string): string {
  return encodeURIComponent(name)
}

export function shortId(id: string): string {
  return id.slice(0, 7)
}

export function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`
}

export function when(unixSeconds: number | null | undefined, lang: string): string {
  if (!unixSeconds) return ''
  return new Date(unixSeconds * 1000).toLocaleString(lang, { dateStyle: 'medium', timeStyle: 'short' })
}

export function whenIso(iso: string | null | undefined, lang: string): string {
  if (!iso) return ''
  return new Date(iso).toLocaleString(lang, { dateStyle: 'medium', timeStyle: 'short' })
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let v = n / 1024
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`
}

/** IFC class without the Ifc prefix, for people: "IfcWallStandardCase" -> "Wall standard case". */
export function ifcClass(t: string): string {
  const bare = t.replace(/^Ifc/, '')
  const words = bare.replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase()
  return words.charAt(0).toUpperCase() + words.slice(1)
}
