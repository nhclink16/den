import { native, invoke, listen } from './native'

export type PickerSource = { id: string; name: string; thumbnail: string; appIcon: string; isScreen: boolean }
export type PickerRequest = { requestId: string; sources: PickerSource[]; audioRequested: boolean; platform: string }

class SharePickerState {
  request = $state<PickerRequest | null>(null)

  attach() {
    if (!native) return
    void listen<PickerRequest>('share-picker', req => { this.request = req })
    void listen<string>('share-picker-closed', id => this.close(id))
  }

  async choose(requestId: string, sourceId: string, audio: boolean) {
    await invoke('share_picker_choose', { requestId, sourceId, audio })
    if (this.request?.requestId === requestId) this.request = null
  }

  async cancel(requestId: string) {
    await invoke('share_picker_choose', { requestId, sourceId: null, audio: false })
    if (this.request?.requestId === requestId) this.request = null
  }

  // Main answers null once the request ended (timeout or a newer request), so close.
  async refreshSources(requestId: string) {
    const sources = await invoke<PickerSource[] | null>('share_picker_sources')
    if (this.request?.requestId !== requestId) return
    this.request = sources ? { ...this.request, sources } : null
  }

  close(requestId: string) {
    if (this.request?.requestId === requestId) this.request = null
  }
}

export const sharePicker = new SharePickerState()
