// A separate store keeps Remove scoped to dictation, including in Tauri webviews.
const database = 'den-dictation-v1'
export const model = 'onnx-community/whisper-tiny.en'
export const revision = '2575352d61be1bf7225cf8f8b268a4678025fc58'
async function open() {
  return new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(database, 1)
    request.onupgradeneeded = () => request.result.createObjectStore('files')
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
  })
}
async function entry(key: string, value?: unknown) {
  const db = await open()
  try {
    return await new Promise<any>((resolve, reject) => {
      const tx = db.transaction('files', value === undefined ? 'readonly' : 'readwrite')
      const request = value === undefined ? tx.objectStore('files').get(key) : tx.objectStore('files').put(value, key)
      tx.oncomplete = () => resolve(request.result)
      tx.onerror = () => reject(tx.error)
      tx.onabort = () => reject(tx.error)
    })
  } finally { db.close() }
}
export const modelCache = {
  async match(key: string) {
    const saved = await entry(key)
    return saved ? new Response(saved.body, { status: saved.status, headers: saved.headers }) : undefined
  },
  async put(key: string, response: Response) {
    await entry(key, { body: await response.arrayBuffer(), status: response.status, headers: [...response.headers] })
  },
}
export async function hasModel() { return !!await entry('ready:'+revision) }
export async function markReady() { await entry('ready:'+revision, true) }
export async function removeModel() {
  const db = await open()
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction('files', 'readwrite'); tx.objectStore('files').clear()
      tx.oncomplete = () => resolve(); tx.onerror = () => reject(tx.error)
    })
  } finally { db.close() }
}
