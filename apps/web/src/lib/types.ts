import type { components } from './schema'
type S = components['schemas']
export type User = S['User']
export type Channel = S['Channel']
export type Category = S['Category']
export type Message = S['Message']
export type Upload = S['Upload']
export type Session = S['Session']
export type Event = S['Event']
export type ApiError = S['ApiError']
export type Invite = S['Invite']
export type Token = S['Token']
export type TokenSecret = S['TokenSecret']
export type BotCreated = S['BotCreated']
export type Reaction = S['Reaction']
export type ChannelReadState = S['ChannelReadState']
export type NotificationPreferences = S['NotificationPreferences']
export type PresenceState = S['PresenceState']

export type CallState = S['CallState']
export type CallToken = S['CallToken']

export type ObjectSummary = S['ObjectSummary']
export type LiveObject = S['Object']
export type ObjectPatch = S['ObjectPatch']
export type Settings = S['Settings']
export type ClientEvent = S['ClientEvent']
export type ObjectVersion = S['ObjectVersion']

export type Host = S['Host']
export type HostEnrollment = S['HostEnrollment']
export type Grant = S['Grant']
export type AccessLog = S['AccessLog']
export type AccessRequest = S['AccessRequest']
export type TerminalState = S['TerminalState']
export type TerminalFrame = S['TerminalFrame']
export type DirectToken = S['DirectToken']
// Until the M10 server half lands, the generated Appearance still carries the single
// `theme` field. These shims describe the shape the client targets; delete them and
// fall back to S['Appearance'] once schema.d.ts is regenerated.
export type AppearanceBackgroundSource =
  | { type: 'builtin'; name: string }
  | { type: 'upload'; id: string }
export type AppearanceBackground = {
  source: AppearanceBackgroundSource
  blur: number
  dim: number
  saturate: number
  scope: 'app' | 'sidebar' | 'chat'
  fit: 'cover' | 'contain' | 'tile'
}
export type Appearance = Omit<S['Appearance'], 'theme'> & {
  light_theme: string
  dark_theme: string
  contrast: number
  background?: AppearanceBackground | null
}
export type Theme = S['Theme']
export type ThemeColors = S['ThemeColors']
export type ThemeFonts = S['ThemeFonts']

export type WsTicket = S['WsTicket']
export type Instance = S['Instance']
