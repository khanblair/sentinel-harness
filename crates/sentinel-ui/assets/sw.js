// Sentinel service worker. It keeps the app shell so the page opens with no network, and it never
// touches the API or the socket: the page itself shows the last state and the offline banner.
const SHELL = "sentinel-shell-v1";

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(SHELL).then((cache) => cache.add("/")));
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) => Promise.all(names.filter((n) => n !== SHELL).map((n) => caches.delete(n))))
      .then(() => self.clients.claim()),
  );
});

function isLive(url) {
  return url.pathname.startsWith("/api/") || url.pathname === "/ws";
}

// Files with a hash in the name never change, so the first copy is the right one.
function isHashed(url) {
  return /-[0-9a-f]{8,}\.(js|wasm|css)$/.test(url.pathname) || url.pathname.startsWith("/fonts/");
}

async function networkFirst(request) {
  const cache = await caches.open(SHELL);
  try {
    const fresh = await fetch(request);
    if (fresh.ok) cache.put(request, fresh.clone());
    return fresh;
  } catch (error) {
    return (await cache.match(request)) || (await cache.match("/")) || Response.error();
  }
}

async function cacheFirst(request) {
  const cache = await caches.open(SHELL);
  const hit = await cache.match(request);
  if (hit) return hit;
  const fresh = await fetch(request);
  if (fresh.ok) cache.put(request, fresh.clone());
  return fresh;
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== self.location.origin || isLive(url)) return;
  event.respondWith(isHashed(url) ? cacheFirst(request) : networkFirst(request));
});
