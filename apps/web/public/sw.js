const CACHE = 'luna-shell-v1'
const SHELL = ['/', '/manifest.webmanifest', '/icon.svg']
self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(SHELL)))
  self.skipWaiting()
})
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))),
      ),
  )
  self.clients.claim()
})
self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url)
  if (event.request.method !== 'GET' || url.pathname.startsWith('/v1/')) return
  event.respondWith(fetch(event.request).catch(() => caches.match(event.request)))
})
