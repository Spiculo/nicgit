// SPDX-License-Identifier: AGPL-3.0-or-later
/** A small (i) control that keeps a field's explanation out of the page until asked. */
export function Info({ text }: { text: string }) {
  return (
    <button type="button" className="info" aria-label={text} data-tip={text}>
      i
    </button>
  )
}
