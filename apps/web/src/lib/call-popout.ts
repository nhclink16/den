// Move the mounted Svelte root, including its event delegation listener. Video,
// canvas and terminal instances survive both directions without another session.
export async function popTile(content: HTMLElement, home: HTMLElement, returned: () => void) {
  const pip = (window as Window & { documentPictureInPicture?: { requestWindow: (options: { width: number; height: number }) => Promise<Window> } }).documentPictureInPicture
  const floating = pip ? await pip.requestWindow({ width: 640, height: 400 }) : window.open('', '', 'popup,width=640,height=400')
  if (!floating) throw new Error('Allow pop-ups for Den, then try again.')
  const doc = floating.document
  doc.title = 'Den · call'
  const base = doc.createElement('base'); base.href = document.baseURI; doc.head.append(base)
  const copied = new Map<Element, Node>()
  function syncStyles() {
    doc.documentElement.style.cssText = document.documentElement.style.cssText
    doc.documentElement.dataset.theme = document.documentElement.dataset.theme
    for (const [source, copy] of copied) if (!source.isConnected) { copy.parentNode?.removeChild(copy); copied.delete(source) }
    for (const node of document.head.querySelectorAll('style,link[rel="stylesheet"]')) {
      const copy = node.cloneNode(true)
      if (copied.has(node)) copied.get(node)!.parentNode?.replaceChild(copy, copied.get(node)!)
      else doc.head.append(copy)
      copied.set(node, copy)
    }
  }
  syncStyles()
  const observer = new MutationObserver(syncStyles); observer.observe(document.head, { childList: true, subtree: true, characterData: true })
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['style', 'data-theme'] })
  const style = doc.createElement('style'); style.textContent = 'html,body{margin:0;width:100%;height:100%;overflow:hidden}body{display:flex;flex-direction:column;background:var(--bg-2)}.pop-back{flex:none;padding:8px;font:11px var(--mono);color:var(--ink-2)}.call-content{flex:1;min-height:0;width:100%;position:relative}.call-content>.tile{width:100%;height:100%;aspect-ratio:auto}.call-content>.canvas-tile{aspect-ratio:16/9;height:auto;max-height:100%;margin:auto}.call-content video{width:100%;height:100%}'
  doc.head.append(style)
  const back = doc.createElement('button'); back.className = 'pop-back'; back.textContent = 'Bring back'
  doc.body.append(back, content)
  const play = () => content.querySelectorAll('video').forEach(v => void v.play().catch(() => {}))
  play()
  let restored = false
  function restore() {
    if (restored) return
    restored = true; observer.disconnect(); floating!.removeEventListener('pagehide', restore)
    home.append(content); play(); returned()
  }
  const close = () => { restore(); floating.close() }
  back.addEventListener('click', close); floating.addEventListener('pagehide', restore)
  return close
}
