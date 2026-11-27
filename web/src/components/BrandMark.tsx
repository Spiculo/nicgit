// SPDX-License-Identifier: AGPL-3.0-or-later
export function BrandMark({ size = 22 }: { size?: number }) {
  return (
    <svg className="brand-mark" viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">
      <rect x="2.5" y="2.5" width="19" height="19" rx="3" fill="none" stroke="currentColor" strokeWidth="1.6" />
      <path d="M7.5 16.5v-9l9 9v-9" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}
