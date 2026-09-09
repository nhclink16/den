// Thin fetch wrapper. Cookie auth in the browser, CSRF header on writes.
import type { ApiError } from './types'

const CSRF_KEY = 'den.csrf'
let csrf: string | null = localStorage.getItem(CSRF_KEY)

export function setCsrf(token: string | null) {
  csrf = token
  if (token) localStorage.setItem(CSRF_KEY, token)
  else localStorage.removeItem(CSRF_KEY)
}

export class HttpError extends Error {
  constructor(public status: number, public code: string, message: string) {
    super(message)
  }
}

async function call<T>(method: string, path: string, body?: unknown, raw?: BodyInit, headers: Record<string, string> = {}): Promise<T> {
  const h: Record<string, string> = { ...headers }
  if (body !== undefined) h['content-type'] = 'application/json'
  if (method !== 'GET' && method !== 'HEAD' && csrf) h['x-csrf-token'] = csrf
  const res = await fetch(path, { method, headers: h, body: body !== undefined ? JSON.stringify(body) : raw, credentials: 'same-origin' })
  if (!res.ok) {
    let err: ApiError = { error: 'http', message: res.statusText }
    try { err = await res.json() } catch { /* not json */ }
    throw new HttpError(res.status, err.error, err.message)
  }
  if (res.status === 204) return undefined as T
  const text = await res.text()
  return (text ? JSON.parse(text) : undefined) as T
}

export const api = {
  get: <T>(path: string) => call<T>('GET', path),
  post: <T>(path: string, body?: unknown) => call<T>('POST', path, body),
  put: <T>(path: string, body?: unknown) => call<T>('PUT', path, body),
  patch: <T>(path: string, body?: unknown) => call<T>('PATCH', path, body),
  patchRaw: <T>(path: string, raw: BodyInit, headers: Record<string, string>) => call<T>('PATCH', path, undefined, raw, { 'content-type': 'application/octet-stream', ...headers }),
  del: <T>(path: string, body?: unknown) => call<T>('DELETE', path, body),
}
