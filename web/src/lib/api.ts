// SPDX-License-Identifier: AGPL-3.0-or-later
import { useQuery, useMutation, useQueryClient, type QueryKey } from '@tanstack/react-query'

export class ApiError extends Error {
  status: number
  code: string
  constructor(status: number, code: string, message: string) {
    super(message)
    this.status = status
    this.code = code
  }
}

export async function api<T = unknown>(path: string, init: RequestInit & { json?: unknown } = {}): Promise<T> {
  const { json, ...rest } = init
  const r = await fetch(path, {
    credentials: 'same-origin',
    ...rest,
    headers: { ...(json !== undefined ? { 'content-type': 'application/json' } : {}), ...(rest.headers ?? {}) },
    body: json !== undefined ? JSON.stringify(json) : rest.body,
  })
  const text = await r.text()
  const data = text ? JSON.parse(text) : null
  if (!r.ok) throw new ApiError(r.status, data?.error ?? 'error', data?.message ?? r.statusText)
  return data as T
}

export function useApi<T>(path: string | null, opts: { refetchInterval?: number } = {}) {
  return useQuery<T, ApiError>({
    queryKey: [path],
    queryFn: () => api<T>(path!),
    enabled: !!path,
    retry: (n, e) => n < 2 && e.status >= 500,
    refetchInterval: opts.refetchInterval,
  })
}

/** A result the engine computes: `pending` while the job runs (the hook polls every second). */
export type JobResult<T> = { pending: true; progress: number; job: string } | { pending: false; data: T }

export function useJob<T>(path: string | null) {
  return useQuery<JobResult<T>, ApiError>({
    queryKey: ['job', path],
    enabled: !!path,
    queryFn: async () => {
      const r = await fetch(path!, { credentials: 'same-origin' })
      const data = await r.json()
      if (r.status === 202) return { pending: true, progress: data.progress ?? 0, job: data.job }
      if (!r.ok) throw new ApiError(r.status, data?.error ?? 'error', data?.message ?? r.statusText)
      return { pending: false, data: data as T }
    },
    refetchInterval: (q) => (q.state.data?.pending ? 1000 : false),
    retry: false,
  })
}

/** Download a version's IFC, waiting while the engine exports it. */
export async function fetchModel(path: string, progress: (fraction: number) => void): Promise<Uint8Array> {
  for (;;) {
    const r = await fetch(path, { credentials: 'same-origin' })
    if (r.status === 202) {
      const d = await r.json()
      progress(d.progress ?? 0)
      await new Promise((res) => setTimeout(res, 1000))
      continue
    }
    if (!r.ok) {
      const d = await r.json().catch(() => ({}))
      throw new ApiError(r.status, d.error ?? 'error', d.message ?? r.statusText)
    }
    return new Uint8Array(await r.arrayBuffer())
  }
}

export function useAction<I, O = unknown>(fn: (input: I) => Promise<O>, invalidate: QueryKey[] = []) {
  const qc = useQueryClient()
  return useMutation<O, ApiError, I>({
    mutationFn: fn,
    onSuccess: () => invalidate.forEach((k) => qc.invalidateQueries({ queryKey: k })),
  })
}

export interface Me {
  id: string
  slug: string
  email: string
  display_name: string
  lang: 'en' | 'es'
  is_site_admin: boolean
  orgs: { slug: string; display_name: string; role: string }[]
}

export function useMe() {
  return useQuery<Me | null, ApiError>({
    queryKey: ['/api/me'],
    queryFn: async () => {
      try {
        return await api<Me>('/api/me')
      } catch (e) {
        if (e instanceof ApiError && e.status === 401) return null
        throw e
      }
    },
    staleTime: 60_000,
  })
}

export function projectApi(owner: string, slug: string, rest = ''): string {
  return `/api/projects/${owner}/${slug}${rest}`
}
