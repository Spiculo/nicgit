// SPDX-License-Identifier: AGPL-3.0-or-later
import type { ReactNode, SVGProps } from 'react'

export type IconName =
  | 'code'
  | 'pull'
  | 'issue'
  | 'eye'
  | 'play'
  | 'image'
  | 'file'
  | 'package'
  | 'bot'
  | 'gear'
  | 'plus'
  | 'chevron'
  | 'signout'
  | 'user'
  | 'key'
  | 'globe'
  | 'building'
  | 'card'
  | 'shield'
  | 'bell'
  | 'trash'
  | 'book'

const PATHS: Record<IconName, ReactNode> = {
  code: <path d="M5.5 4 1.8 8l3.7 4M10.5 4l3.7 4-3.7 4" />,
  pull: (
    <>
      <circle cx="4" cy="3.2" r="1.5" />
      <circle cx="4" cy="12.8" r="1.5" />
      <circle cx="12" cy="12.8" r="1.5" />
      <path d="M4 4.7v6.6M12 11.3V8a3 3 0 0 0-3-3H8.5" />
    </>
  ),
  issue: (
    <>
      <circle cx="8" cy="8" r="6" />
      <circle cx="8" cy="8" r="1.2" fill="currentColor" stroke="none" />
    </>
  ),
  eye: (
    <>
      <path d="M1.5 8S4 3.8 8 3.8 14.5 8 14.5 8 12 12.2 8 12.2 1.5 8 1.5 8Z" />
      <circle cx="8" cy="8" r="2" />
    </>
  ),
  play: <path d="M5.2 3.4 12.4 8l-7.2 4.6Z" />,
  image: (
    <>
      <rect x="2" y="3" width="12" height="10" rx="1" />
      <circle cx="5.6" cy="6.4" r="1.1" />
      <path d="m2.8 11.6 3-3 2.2 2.2 2-2 3.2 3.2" />
    </>
  ),
  file: (
    <>
      <path d="M4 1.8h5l3 3v9.4H4z" />
      <path d="M9 1.8v3h3" />
    </>
  ),
  package: (
    <>
      <path d="m8 1.8 5.5 3v6.4L8 14.2l-5.5-3V4.8z" />
      <path d="M2.5 4.8 8 7.8l5.5-3M8 7.8v6.4" />
    </>
  ),
  bot: (
    <>
      <rect x="3" y="5" width="10" height="8" rx="2" />
      <path d="M8 2v3M6 8.5h.01M10 8.5h.01M6 11h4" />
    </>
  ),
  gear: (
    <>
      <circle cx="8" cy="8" r="2.2" />
      <path d="M8 1.5v1.7M8 12.8v1.7M1.5 8h1.7M12.8 8h1.7M3.4 3.4l1.2 1.2M11.4 11.4l1.2 1.2M12.6 3.4l-1.2 1.2M4.6 11.4l-1.2 1.2" />
    </>
  ),
  plus: <path d="M8 2.5v11M2.5 8h11" />,
  chevron: <path d="m4 6 4 4 4-4" />,
  signout: <path d="M6 2.5H3.2v11H6M9.5 5 12.5 8l-3 3M12.5 8H6.5" />,
  user: (
    <>
      <circle cx="8" cy="5" r="2.5" />
      <path d="M3 13.5c0-2.5 2.2-4 5-4s5 1.5 5 4" />
    </>
  ),
  key: (
    <>
      <circle cx="5" cy="10.5" r="2.5" />
      <path d="m6.8 8.7 4.7-4.7M10 3l2 2M9 4l2 2" />
    </>
  ),
  globe: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="M2 8h12M8 2c1.7 1.7 2.5 3.7 2.5 6S9.7 12.3 8 14c-1.7-1.7-2.5-3.7-2.5-6S6.3 3.7 8 2Z" />
    </>
  ),
  building: (
    <>
      <path d="M3 14V2.5h6V14M9 6.5h4V14M1.5 14h13" />
      <path d="M5 5h2M5 7.5h2M5 10h2M11 9h.5M11 11.5h.5" />
    </>
  ),
  card: (
    <>
      <rect x="1.5" y="3.5" width="13" height="9" rx="1.5" />
      <path d="M1.5 6.5h13M4 9.5h2" />
    </>
  ),
  shield: (
    <>
      <path d="M8 1.8 13 4v3.7c0 3.2-2 5.4-5 6.5-3-1.1-5-3.3-5-6.5V4z" />
      <path d="m5.8 8 1.6 1.6 3-3.2" />
    </>
  ),
  bell: <path d="M4 6.5a4 4 0 0 1 8 0c0 4 1.2 4.8 1.5 5.5H2.5C2.8 11.3 4 10.5 4 6.5ZM6.5 12a1.5 1.5 0 0 0 3 0" />,
  trash: <path d="M3 4.5h10M6 4.5V3h4v1.5M4.5 4.5l.6 9h5.8l.6-9M6.8 7v4M9.2 7v4" />,
  book: (
    <>
      <path d="M3 2.5h8.5v11H3z" />
      <path d="M5.5 2.5v11" />
    </>
  ),
}

export function Icon({
  name,
  size = 16,
  ...rest
}: { name: IconName; size?: number } & SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 16 16"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className="icon"
      {...rest}
    >
      {PATHS[name]}
    </svg>
  )
}
