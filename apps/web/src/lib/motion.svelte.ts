// Animated pictures sit still until someone shows interest in them. The server marks
// a GIF's URL with `v=a_` (Discord's convention) and serves its first frame on `still`.
import { MediaQuery } from 'svelte/reactivity'

export const reducedMotion = new MediaQuery('prefers-reduced-motion: reduce')
export const isAnimated = (url: string | null | undefined) => !!url && /[?&]v=a_/.test(url)
export const stillUrl = (url: string) => `${url}${url.includes('?') ? '&' : '?'}still=1`
