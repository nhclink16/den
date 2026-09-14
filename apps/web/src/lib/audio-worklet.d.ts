// AudioWorkletGlobalScope is absent from TypeScript's window/worker DOM libraries.
declare class AudioWorkletProcessor { readonly port: MessagePort }
declare const sampleRate: number
declare function registerProcessor(name: string, processor: typeof AudioWorkletProcessor): void

declare module '*?url&no-inline' { const url: string; export default url }
