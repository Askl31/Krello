// Client API REST — fetch JSON avec gestion d'erreurs.

async function req(method, path, body, opts = {}) {
  const headers = {};
  if (body !== undefined && !(body instanceof FormData)) headers['content-type'] = 'application/json';
  let payload;
  if (body instanceof FormData) payload = body;
  else if (body !== undefined) payload = JSON.stringify(body);
  const res = await fetch(path, { method, headers, body: payload, ...opts });
  if (!res.ok) {
    let msg = `${res.status} ${res.statusText}`;
    try {
      const data = await res.json();
      if (data && data.message) msg = data.message;
    } catch { /* ignore */ }
    const err = new Error(msg);
    err.status = res.status;
    throw err;
  }
  if (res.status === 204) return null;
  const ct = res.headers.get('content-type') || '';
  if (ct.includes('application/json')) return res.json();
  return res.text();
}

export const api = {
  get: (path) => req('GET', path),
  post: (path, body) => req('POST', path, body),
  patch: (path, body) => req('PATCH', path, body),
  put: (path, body) => req('PUT', path, body),
  del: (path) => req('DELETE', path),

  boards: () => api.get('/api/v1/boards'),
  board: (id) => api.get(`/api/v1/boards/${id}`),
  createBoard: (b) => api.post('/api/v1/boards', b),
  updateBoard: (id, p) => api.patch(`/api/v1/boards/${id}`, p),
  deleteBoard: (id) => api.del(`/api/v1/boards/${id}`),
  deleteAllBoards: () => api.del('/api/v1/boards'),
  duplicateBoard: (id) => api.post(`/api/v1/boards/${id}/duplicate`),
  boardActivity: (id, limit = 60, cardId) =>
    api.get(`/api/v1/boards/${id}/activity?limit=${limit}${cardId ? `&card_id=${cardId}` : ''}`),

  lists: (boardId) => api.get(`/api/v1/boards/${boardId}/lists`),
  createList: (boardId, l) => api.post(`/api/v1/boards/${boardId}/lists`, l),
  duplicateList: (id) => api.post(`/api/v1/lists/${id}/duplicate`),
  updateList: (id, p) => api.patch(`/api/v1/lists/${id}`, p),
  deleteList: (id) => api.del(`/api/v1/lists/${id}`),

  cards: (boardId, includeArchived = false) =>
    api.get(`/api/v1/boards/${boardId}/cards?include_archived=${includeArchived}`),
  card: (id) => api.get(`/api/v1/cards/${id}`),
  cardsMeta: (boardId) => api.get(`/api/v1/boards/${boardId}/cards/meta`),
  createCard: (boardId, c) => api.post(`/api/v1/boards/${boardId}/cards`, c),
  updateCard: (id, p) => api.patch(`/api/v1/cards/${id}`, p),
  deleteCard: (id) => api.del(`/api/v1/cards/${id}`),
  moveCard: (id, m) => api.post(`/api/v1/cards/${id}/move`, m),
  duplicateCard: (id) => api.post(`/api/v1/cards/${id}/duplicate`),
  setCardTemplate: (id, p) => api.patch(`/api/v1/cards/${id}/template`, p),
  instantiateTemplate: (id, p) => api.post(`/api/v1/cards/${id}/instantiate`, p),
  boardTemplates: (boardId) => api.get(`/api/v1/boards/${boardId}/templates`),
  setCardLabels: (id, labelIds) => api.put(`/api/v1/cards/${id}/labels`, labelIds),
  attachLabel: (id, labelId) => api.post(`/api/v1/cards/${id}/labels/${labelId}`),
  setCover: (id, color) => api.post(`/api/v1/cards/${id}/cover`, { color }),
  setFieldValue: (cardId, fieldId, value) => api.put(`/api/v1/cards/${cardId}/fields/${fieldId}`, { value }),

  labels: (boardId) => api.get(`/api/v1/boards/${boardId}/labels`),
  createLabel: (boardId, l) => api.post(`/api/v1/boards/${boardId}/labels`, l),
  updateLabel: (id, p) => api.patch(`/api/v1/labels/${id}`, p),
  deleteLabel: (id) => api.del(`/api/v1/labels/${id}`),

  createChecklist: (cardId, c) => api.post(`/api/v1/cards/${cardId}/checklists`, c),
  deleteChecklist: (id) => api.del(`/api/v1/checklists/${id}`),
  createChecklistItem: (clId, c) => api.post(`/api/v1/checklists/${clId}/items`, c),
  updateChecklistItem: (id, p) => api.patch(`/api/v1/checklist-items/${id}`, p),
  deleteChecklistItem: (id) => api.del(`/api/v1/checklist-items/${id}`),

  comments: (cardId) => api.get(`/api/v1/cards/${cardId}/comments`),
  addComment: (cardId, body) => api.post(`/api/v1/cards/${cardId}/comments`, { body }),
  deleteComment: (id) => api.del(`/api/v1/comments/${id}`),

  attachments: (cardId) => api.get(`/api/v1/cards/${cardId}/attachments`),
  uploadAttachment: (cardId, file) => {
    const fd = new FormData();
    fd.append('file', file);
    return api.post(`/api/v1/cards/${cardId}/attachments`, fd);
  },
  deleteAttachment: (id) => api.del(`/api/v1/attachments/${id}`),
  setAttachmentCover: (id) => api.post(`/api/v1/attachments/${id}/cover`),

  fields: (boardId) => api.get(`/api/v1/boards/${boardId}/fields`),
  createField: (boardId, f) => api.post(`/api/v1/boards/${boardId}/fields`, f),
  updateField: (id, p) => api.patch(`/api/v1/fields/${id}`, p),
  deleteField: (id) => api.del(`/api/v1/fields/${id}`),

  deps: (boardId) => api.get(`/api/v1/boards/${boardId}/dependencies`),
  addDep: (boardId, d) => api.post(`/api/v1/boards/${boardId}/dependencies`, d),
  updateDep: (id, p) => api.patch(`/api/v1/dependencies/${id}`, p),
  deleteDep: (id) => api.del(`/api/v1/dependencies/${id}`),
  schedule: (boardId) => api.get(`/api/v1/boards/${boardId}/schedule`),
  recompute: (boardId) => api.post(`/api/v1/boards/${boardId}/schedule`),

  baselines: (boardId) => api.get(`/api/v1/boards/${boardId}/baselines`),
  createBaseline: (boardId, name) => api.post(`/api/v1/boards/${boardId}/baselines`, { name }),
  deleteBaseline: (id) => api.del(`/api/v1/baselines/${id}`),
  compareBaseline: (id) => api.get(`/api/v1/baselines/${id}/compare`),

  automations: (boardId) => api.get(`/api/v1/boards/${boardId}/automations`),
  createAutomation: (boardId, a) => api.post(`/api/v1/boards/${boardId}/automations`, a),
  updateAutomation: (id, p) => api.patch(`/api/v1/automations/${id}`, p),
  deleteAutomation: (id) => api.del(`/api/v1/automations/${id}`),
  runAutomation: (id) => api.post(`/api/v1/automations/${id}/run`),
  automationRuns: (id, limit = 50) => api.get(`/api/v1/automations/${id}/runs?limit=${limit}`),
  boardAutomationRuns: (boardId, limit = 100) => api.get(`/api/v1/boards/${boardId}/automations/runs?limit=${limit}`),

  calendars: () => api.get('/api/v1/calendars'),
  createCalendar: (c) => api.post('/api/v1/calendars', c),
  updateCalendar: (id, p) => api.patch(`/api/v1/calendars/${id}`, p),
  deleteCalendar: (id) => api.del(`/api/v1/calendars/${id}`),

  uiSettings: () => api.get('/api/v1/settings/ui'),
  saveUiSettings: (s) => api.put('/api/v1/settings/ui', s),

  backupSettings: () => api.get('/api/v1/settings/backup'),
  saveBackupSettings: (s) => api.put('/api/v1/settings/backup', s),
  runBackup: () => api.post('/api/v1/settings/backup/run'),
  pickBackupDir: () => api.post('/api/v1/settings/backup/pick'),

  search: (q, boardId) =>
    api.get(`/api/v1/search?q=${encodeURIComponent(q)}${boardId ? `&board_id=${boardId}` : ''}`),

  exportUrl: (boardId, format) => `/api/v1/boards/${boardId}/export?format=${format}`,
  importTrello: (text) => req('POST', '/api/v1/import/trello', text),
  importKrello: (text) => req('POST', '/api/v1/import/krello', text),

  undo: () => api.post('/api/v1/undo'),
  redo: () => api.post('/api/v1/redo'),

  aiStatus: () => api.get('/api/v1/ai/status'),
  aiParseTask: (input, listId, create) => api.post('/api/v1/ai/parse-task', { input, list_id: listId, create }),
  aiPlan: (goal, boardId, listId, apply) => api.post('/api/v1/ai/plan', { goal, board_id: boardId, list_id: listId, apply }),
  aiSummarize: (boardId) => api.post('/api/v1/ai/summarize', { board_id: boardId }),
  aiPriorities: (boardId, apply) => api.post('/api/v1/ai/priorities', { board_id: boardId, apply }),
  aiRewrite: (text) => api.post('/api/v1/ai/rewrite', { text }),
  aiChecklist: (cardId, apply) => api.post('/api/v1/ai/checklist', { card_id: cardId, apply }),

  saveViewSettings: (boardId, view, settings) =>
    api.put(`/api/v1/boards/${boardId}/views/${view}`, { settings }),
};
