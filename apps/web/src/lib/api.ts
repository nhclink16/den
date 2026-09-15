// Thin fetch wrapper. Cookie auth in the browser, CSRF header on writes.
import { native, invoke, activeOrigin } from './native'
import type { ApiError } from './types'

const CSRF_KEY = 'den.csrf'
let csrf: string | null = localStorage.getItem(CSRF_KEY)

export function setCsrf(token: string | null) {
  if (native) return
  csrf = token
  if (token) localStorage.setItem(CSRF_KEY, token)
  else localStorage.removeItem(CSRF_KEY)
}

export class HttpError extends Error {
  constructor(public status: number, public code: string, message: string) {
    super(message)
  }
}

async function call<T>(origin: string, method: string, path: string, body?: unknown, raw?: BodyInit, headers: Record<string, string> = {}): Promise<T> {
  const h: Record<string, string> = { ...headers }
  if (body !== undefined) h['content-type'] = 'application/json'
  if (method !== 'GET' && method !== 'HEAD' && csrf) h['x-csrf-token'] = csrf
  let res: Response
  if (native) {
    const bytes = body !== undefined ? new TextEncoder().encode(JSON.stringify(body)) : raw ? new Uint8Array(await new Response(raw).arrayBuffer()) : undefined
    const response = await invoke<{ status: number; headers: Record<string, string>; body: number[] }>('api_request', { origin, method, path, body: bytes ? Array.from(bytes) : null, headers: h })
    res = new Response(response.status === 204 ? null : new Uint8Array(response.body), { status: response.status, headers: response.headers })
  } else res = await fetch(path, { method, headers: h, body: body !== undefined ? JSON.stringify(body) : raw, credentials: 'same-origin' })
  if (!res.ok) {
    let err: ApiError = { error: 'http', message: res.statusText }
    try { err = await res.json() } catch { /* not json */ }
    if (method !== 'GET' && method !== 'HEAD') window.dispatchEvent(new CustomEvent('den-sound-event', { detail: { origin, sound: 'error' } }))
    throw new HttpError(res.status, err.error, err.message)
  }
  if (res.status === 204) return undefined as T
  const text = await res.text()
  return (text ? JSON.parse(text) : undefined) as T
}

export const apiFor = (origin: string) => ({
  get: <T>(path: string) => call<T>(origin, 'GET', path),
  post: <T>(path: string, body?: unknown) => call<T>(origin, 'POST', path, body),
  put: <T>(path: string, body?: unknown) => call<T>(origin, 'PUT', path, body),
  patch: <T>(path: string, body?: unknown) => call<T>(origin, 'PATCH', path, body),
  patchRaw: <T>(path: string, raw: BodyInit, headers: Record<string, string>) => call<T>(origin, 'PATCH', path, undefined, raw, { 'content-type': 'application/octet-stream', ...headers }),
  putRaw: <T>(path: string, raw: BodyInit, headers: Record<string, string>) => call<T>(origin, 'PUT', path, undefined, raw, { 'content-type': 'application/octet-stream', ...headers }),
  del: <T>(path: string, body?: unknown) => call<T>(origin, 'DELETE', path, body),
})
export const api = new Proxy({} as ReturnType<typeof apiFor>, {
  get: (_target, key: keyof ReturnType<typeof apiFor>) => apiFor(activeOrigin())[key],
})

/** Binary account media uses the same authenticated native bridge as JSON requests. */
export async function fetchBytes(origin: string, path: string): Promise<ArrayBuffer> {
  if (native) {
    const response = await invoke<{ status: number; body: number[] }>('api_request', { origin, method: 'GET', path, body: null, headers: {} })
    if (response.status !== 200) throw Error('Sound unavailable')
    return new Uint8Array(response.body).buffer
  }
  const response = await fetch(path, { credentials: 'same-origin' })
  if (!response.ok) throw Error('Sound unavailable')
  return response.arrayBuffer()
}
