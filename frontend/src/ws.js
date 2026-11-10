// WebSocket reconnectant + bus d'événements applicatif.

const listeners = new Map();

export function on(type, fn) {
  if (!listeners.has(type)) listeners.set(type, new Set());
  listeners.get(type).add(fn);
  return () => listeners.get(type)?.delete(fn);
}

export function emit(type, data) {
  listeners.get(type)?.forEach((fn) => fn(data));
  listeners.get('*')?.forEach((fn) => fn({ type, data }));
}

let ws = null;
let retries = 0;
let closed = false;

function connect() {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  ws = new WebSocket(`${proto}://${location.host}/api/v1/ws`);
  ws.onopen = () => {
    retries = 0;
    emit('ws.open');
  };
  ws.onmessage = (ev) => {
    try {
      const evt = JSON.parse(ev.data);
      if (evt && evt.type) emit(evt.type, evt.data);
    } catch { /* ignore */ }
  };
  ws.onclose = () => {
    if (closed) return;
    retries += 1;
    setTimeout(connect, Math.min(1500 * retries, 8000));
  };
  ws.onerror = () => ws.close();
}

export function startWs() {
  closed = false;
  connect();
  window.addEventListener('beforeunload', () => { closed = true; ws?.close(); });
}
