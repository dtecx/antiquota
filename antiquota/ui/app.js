'use strict';

/* antiquota GUI: sessions on the left, the relay of accounts below them, one
   transcript in the middle, a composer docked at the bottom. Everything live
   arrives over one EventSource; everything else is a small JSON call. */

const TOKEN = document.querySelector('meta[name="antiquota-token"]').content;
const $ = (s, el = document) => el.querySelector(s);
/* Inside the desktop app (Tauri), a few browser features go through native APIs instead. */
const NATIVE = window.__TAURI__ || null;
const openExternal = url => NATIVE ? NATIVE.opener.openUrl(url) : window.open(url, '_blank', 'noopener');
const readClipboard = () => NATIVE ? NATIVE.clipboardManager.readText() : navigator.clipboard.readText();
const copyText = text => NATIVE ? NATIVE.clipboardManager.writeText(text) : navigator.clipboard.writeText(text);
/* In-app confirmation: the same everywhere (the app's web view may not show window.confirm). */
function ask(message, { ok = 'Continue', danger = false, title = '' } = {}) {
  return new Promise(resolve => {
    const dlg = h('dialog', { class: 'dlg ask', 'aria-label': title || 'Confirm' });
    const done = v => { dlg.close(); dlg.remove(); resolve(v); };
    dlg.append(h('form', { method: 'dialog', onsubmit: e => { e.preventDefault(); done(true); } },
      title ? h('h2', {}, title) : null,
      h('p', { class: 'ask-msg' }, message),
      h('div', { class: 'actions' },
        h('button', { type: 'button', class: 'ghost', onclick: () => done(false) }, 'Cancel'),
        h('button', { type: 'submit', class: danger ? 'primary danger' : 'primary', autofocus: '' }, ok))));
    dlg.addEventListener('cancel', e => { e.preventDefault(); done(false); });
    document.body.append(dlg);
    dlg.showModal();
  });
}
async function saveText(name, text, type = 'text/markdown') {
  if (NATIVE) {
    const where = await NATIVE.core.invoke('save_file', { name, contents: text });
    if (where) toast('Saved ' + where);
    return;
  }
  const a = h('a', { href: URL.createObjectURL(new Blob([text], { type })), download: name });
  document.body.append(a);
  a.click();
  a.remove();
}
const $$ = (s, el = document) => [...el.querySelectorAll(s)];

function h(tag, attrs = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'html') el.innerHTML = v;
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of kids.flat(Infinity)) if (c != null && c !== false) el.append(c.nodeType ? c : String(c));
  return el;
}

const S = {
  sessions: new Map(), cur: null, accounts: [], models: [], defaults: {},
  nodes: new Map(), live: new Map(), drawer: false, tab: 'accounts', stack: null, detail: 'detailed', events: [],
};
try {
  S.drawer = localStorage.getItem('antiquota.drawer') === '1';
  S.tab = localStorage.getItem('antiquota.tab') || 'accounts';
  S.detail = localStorage.getItem('antiquota.detail') || 'detailed';
} catch {}
function remember(k, v) { try { localStorage.setItem('antiquota.' + k, v); } catch {} }

async function api(method, path, body) {
  const r = await fetch('/api/' + path, {
    method,
    headers: { 'X-Antiquota-Token': TOKEN, ...(body ? { 'Content-Type': 'application/json' } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  let j = {};
  try { j = await r.json(); } catch {}
  if (!r.ok) throw new Error(j.error || r.statusText);
  return j;
}

let toastTimer;
function toast(msg) {
  const t = $('#toast');
  t.textContent = msg; t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { t.hidden = true; }, 2600);
}
const fail = e => toast(e.message || String(e));

/* ---------- formatting ---------- */
const esc = s => String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const fmtK = n => n >= 1e6 ? (n / 1e6).toFixed(1) + 'M' : n >= 1e3 ? (n / 1e3).toFixed(1) + 'k' : String(n || 0);
const clock = t => t ? new Date(t * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false }) : '';
const secs = s => s == null ? '' : s < 1 ? '<1s' : s < 60 ? s.toFixed(s < 10 ? 1 : 0) + 's' : Math.floor(s / 60) + 'm' + String(Math.round(s % 60)).padStart(2, '0');
function ago(t) {
  const d = Date.now() / 1000 - t;
  if (d < 60) return 'now';
  if (d < 3600) return Math.floor(d / 60) + 'm';
  if (d < 86400) return Math.floor(d / 3600) + 'h';
  return new Date(t * 1000).toLocaleDateString([], { month: 'short', day: 'numeric' });
}
const cur = () => S.sessions.get(S.cur);
const ATT = [];  // attachments uploaded but not sent yet: {name, path, size, image, url}
const who = email => String(email || '').split('@')[0];
function rel(p) {
  const m = cur();
  if (!p || !m) return p || '';
  return p.startsWith(m.cwd + '/') ? p.slice(m.cwd.length + 1) : p;
}

/* Markdown, safely: everything is escaped first, then a small known grammar is
   turned back into tags. Model text can never inject markup or script. */
function inline(s) {
  const codes = [];
  s = s.replace(/`([^`\n]+)`/g, (_, c) => { codes.push(c); return '\u0000' + (codes.length - 1) + '\u0000'; });
  s = esc(s)
    .replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*\w])\*([^*\n]+)\*(?!\w)/g, '$1<em>$2</em>')
    .replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/g, (m, text, url) => {
      if (/^https?:\/\//i.test(url)) return `<a href="${url}" target="_blank" rel="noopener noreferrer">${text}</a>`;
      if (/^file:\/\//i.test(url)) return `<a class="path" href="#" data-file="${url.replace(/^file:\/\//i, '')}" title="Open ${url.replace(/^file:\/\//i, '')}">${text}</a>`;
      return text;
    })
    .replace(/\n/g, '<br>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, n) => `<code>${esc(codes[n])}</code>`);
}
function md(src) {
  const L = String(src).replace(/\r/g, '').split('\n');
  const block = /^(```|#{1,4}\s|>|\s*([-*+]|\d+\.)\s|\|)/;
  let out = '', i = 0;
  while (i < L.length) {
    const l = L[i];
    if (/^```/.test(l)) {
      const buf = [];
      i++;
      while (i < L.length && !/^```/.test(L[i])) buf.push(L[i++]);
      i++;
      out += `<pre class="code"><code>${esc(buf.join('\n'))}</code></pre>`;
    } else if (/^#{1,4}\s/.test(l)) {
      const n = l.match(/^#+/)[0].length + 2;
      out += `<h${n}>${inline(l.replace(/^#+\s*/, ''))}</h${n}>`;
      i++;
    } else if (/^\s*([-*+]|\d+\.)\s+/.test(l)) {
      const ordered = /^\s*\d+\./.test(l), items = [];
      while (i < L.length && /^\s*([-*+]|\d+\.)\s+/.test(L[i])) items.push(L[i++].replace(/^\s*([-*+]|\d+\.)\s+/, ''));
      const tag = ordered ? 'ol' : 'ul';
      out += `<${tag}>${items.map(x => `<li>${inline(x)}</li>`).join('')}</${tag}>`;
    } else if (/^>/.test(l)) {
      const q = [];
      while (i < L.length && /^>/.test(L[i])) q.push(L[i++].replace(/^>\s?/, ''));
      out += `<blockquote>${inline(q.join('\n'))}</blockquote>`;
    } else if (/^\|.*\|\s*$/.test(l) && /^\|[\s:|-]+\|\s*$/.test(L[i + 1] || '')) {
      const row = r => r.trim().replace(/^\||\|$/g, '').split('|').map(c => c.trim());
      const head = row(l);
      i += 2;
      const rows = [];
      while (i < L.length && /^\|.*\|\s*$/.test(L[i])) rows.push(row(L[i++]));
      out += `<table><thead><tr>${head.map(c => `<th>${inline(c)}</th>`).join('')}</tr></thead><tbody>${
        rows.map(r => `<tr>${r.map(c => `<td>${inline(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
    } else if (!l.trim()) {
      i++;
    } else {
      const p = [];
      while (i < L.length && L[i].trim() && !block.test(L[i])) p.push(L[i++]);
      if (!p.length) p.push(L[i++]);
      out += `<p>${inline(p.join('\n'))}</p>`;
    }
  }
  return out;
}
function addCopy(node) {
  for (const pre of $$('pre.code', node)) {
    if ($('.copy', pre)) continue;
    pre.append(h('button', {
      class: 'copy', type: 'button',
      onclick: () => copyText($('code', pre).textContent).then(() => toast('Copied')),
    }, 'copy'));
  }
}

/* ---------- icons ---------- */
const svg = d => `<svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
const ICON = {
  run: svg('<path d="M3 4.5 6.5 8 3 11.5M8 12h5"/>'),
  read: svg('<path d="M4 1.8h5l3 3v9.4H4z"/><path d="M9 1.8v3h3"/>'),
  edit: svg('<path d="M10.5 2.5l3 3-8 8H2.5v-3z"/>'),
  search: svg('<circle cx="7" cy="7" r="4.2"/><path d="m10.2 10.2 3.3 3.3"/>'),
  list: svg('<path d="M2 4h3l1.5 1.5H14v8H2z"/>'),
  web: svg('<circle cx="8" cy="8" r="6"/><path d="M2 8h12M8 2c2 2 2 10 0 12M8 2c-2 2-2 10 0 12"/>'),
  mcp: svg('<path d="M6 2v3M10 2v3M4 5h8v3a4 4 0 0 1-8 0zM8 12v2"/>'),
  other: svg('<circle cx="8" cy="8" r="1.6" fill="currentColor"/>'),
};

/* ---------- sessions list ---------- */
function sessItem(m) {
  const g = m.goal;
  const goalOn = g && g.state === 'active';
  let sub = '';
  if (m.status === 'sleeping') sub = 'sleeping until ' + clock(m.status_until);
  else if (m.status === 'approval') sub = '● needs your approval';
  else if (g) sub = `◎ ${g.state === 'active' ? 'round ' + g.round + '/' + g.max_rounds : g.state}`;
  return h('div', { class: 'sess-row', oncontextmenu: e => { e.preventDefault(); sessMenu(m, e.clientX, e.clientY); } },
    h('button', {
      class: 'sess' + (m.id === S.cur ? ' on' : '') + (m.archived ? ' archived' : ''), title: m.title || 'Untitled',
      onclick: () => openSession(m.id),
    },
    h('span', { class: `dot ${m.status}${goalOn ? ' goal' : ''}` }),
    h('span', { class: 't' }, (m.pinned ? '◆ ' : '') + (m.worktree ? '⎇ ' : '') + (m.title || 'Untitled')),
    h('span', { class: 'when' }, ago(m.updated)),
    sub && h('span', { class: 'sub' + (m.status === 'approval' ? ' attn' : '') }, sub)),
    h('button', {
      class: 'sess-more', title: 'More', 'aria-label': 'Session actions',
      onclick: e => { const r = e.currentTarget.getBoundingClientRect(); sessMenu(m, r.left, r.bottom + 4); },
    }, '⋯'));
}
function renderSessions() {
  const q = $('#search').value.trim().toLowerCase();
  const all = [...S.sessions.values()];
  const archived = all.filter(m => m.archived).length;
  const list = all
    .filter(m => (S.showArchived || !m.archived || m.id === S.cur))
    .filter(m => !q || (m.title || '').toLowerCase().includes(q) || m.cwd.toLowerCase().includes(q))
    .sort((a, b) => b.updated - a.updated);
  const groups = new Map();
  const pinned = list.filter(m => m.pinned);
  if (pinned.length) groups.set('\u0000pinned', pinned);
  for (const m of list.filter(m => !m.pinned)) {
    const key = m.worktree ? m.worktree.repo : m.cwd;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(m);
  }
  const nav = $('#sessions');
  nav.replaceChildren();
  if (!list.length) nav.append(h('div', { class: 'relay-empty' }, q ? 'No title matches.' : 'No sessions yet.'));
  for (const [cwd, ms] of groups) {
    const name = cwd === '\u0000pinned' ? 'Pinned' : cwd.split(/[\\/]/).pop() || cwd;
    nav.append(h('div', { class: 'group' }, h('div', { class: 'group-name', title: cwd === '\u0000pinned' ? '' : cwd }, name),
      ms.map(sessItem)));
  }
  if (q.length >= 2) nav.append(h('div', { class: 'group', id: 'found' }, h('div', { class: 'group-name' }, 'In messages'),
    h('div', { class: 'relay-empty' }, 'Searching…')));
  if (archived) nav.append(h('button', { class: 'show-archived', onclick: () => { S.showArchived = !S.showArchived; renderSessions(); } },
    S.showArchived ? 'Hide archived' : `Show archived (${archived})`));
  if (q.length >= 2) searchMessages(q);
}
let searchSeq = 0;
async function searchMessages(q) {
  const seq = ++searchSeq;
  await new Promise(r => setTimeout(r, 220));
  if (seq !== searchSeq) return;
  let hits = [];
  try { hits = await api('GET', 'search?q=' + encodeURIComponent(q)); } catch { return; }
  const box = $('#found');
  if (seq !== searchSeq || !box) return;
  box.replaceChildren(h('div', { class: 'group-name' }, 'In messages'),
    ...(hits.length ? hits.map(x => h('button', { class: 'sess found', title: x.cwd, onclick: () => openSession(x.id) },
      h('span', { class: 'dot' }), h('span', { class: 't' }, x.title || 'Untitled'), h('span', { class: 'when' }, ago(x.ts)),
      h('span', { class: 'sub' }, (x.who === 'user' ? 'you: ' : 'agy: ') + x.snippet))) : [h('div', { class: 'relay-empty' }, 'Nothing found.')]));
}
/* Right-click or ⋯ on a session. */
function sessMenu(m, x, y) {
  closeMenu();
  const act = (label, fn, cls = '') => h('button', { class: cls, role: 'menuitem', onclick: () => { closeMenu(); fn(); } }, label);
  const menu = h('div', { class: 'ctxmenu', id: 'ctx', role: 'menu' },
    act('Rename', async () => { await openSession(m.id); $('#title').click(); }),
    act(m.pinned ? 'Unpin' : 'Pin to top', () => api('PATCH', 'sessions/' + m.id, { pinned: !m.pinned }).catch(fail)),
    act('Fork: new session from its state', async () => {
      try { const f = await api('POST', `sessions/${m.id}/fork`); S.sessions.set(f.id, f); await openSession(f.id); toast('Forked: the next message starts from its handoff state'); } catch (e) { fail(e); }
    }),
    act(m.archived ? 'Unarchive' : 'Archive', () => api('PATCH', 'sessions/' + m.id, { archived: !m.archived }).catch(fail)),
    h('hr'),
    act('Delete…', async () => {
      if (await ask('The transcript and its handoff state are deleted. Files in the project stay.',
        { title: `Delete "${m.title || 'Untitled'}"?`, ok: 'Delete', danger: true })) api('DELETE', 'sessions/' + m.id).catch(fail);
    }, 'danger'));
  document.body.append(menu);
  const r = menu.getBoundingClientRect();
  menu.style.left = Math.min(x, innerWidth - r.width - 8) + 'px';
  menu.style.top = Math.min(y, innerHeight - r.height - 8) + 'px';
  menu.querySelector('button').focus();
  setTimeout(() => document.addEventListener('mousedown', outsideMenu), 0);
}
function outsideMenu(e) { if (!e.target.closest('#ctx')) closeMenu(); }
function closeMenu() { $('#ctx')?.remove(); document.removeEventListener('mousedown', outsideMenu); }

/* ---------- the relay: accounts and their quota ---------- */
function minFrac(windows) {
  const v = Object.values(windows || {}).map(w => w[0]);
  return v.length ? Math.min(...v) : null;
}
function gauge(letter, acct, bucket) {
  const wall = acct.walls[bucket], frac = minFrac(acct.quota[bucket]);
  const lab = wall ? '↺' + clock(wall) : frac == null ? '–' : Math.round(frac * 100) + '%';
  return h('div', { class: 'gauge' },
    h('div', { class: 'lab' }, h('span', {}, letter), h('span', {}, lab)),
    h('div', { class: 'bar' + (wall ? ' wall' : frac != null && frac < .2 ? ' low' : '') },
      h('i', { style: `width:${frac == null ? 0 : Math.max(2, frac * 100)}%` })));
}
function renderRelay() {
  const rows = $('#relay-rows');
  rows.replaceChildren();
  if (!S.accounts.length) rows.append(h('button', { class: 'primary relay-cta', onclick: () => openAcct() }, 'Add Google account'));
  for (const acct of S.accounts) {
    rows.append(acct.verify
      ? h('div', { class: 'acct verify', title: `${acct.slot}: Google wants this account verified before Antigravity serves it` },
        h('span', { class: 'name' }, who(acct.slot)),
        h('button', { class: 'verify-chip', onclick: () => showDrawer('accounts') }, 'needs verification'))
      : h('div', { class: 'acct' + (acct.paused ? ' paused' : ''), title: acct.paused ? acct.slot + ' is paused' : acct.busy ? acct.slot + ' is running a call' : acct.slot },
        h('span', { class: 'name' + (acct.busy ? ' busy' : '') }, who(acct.slot)),
        gauge('gemini', acct, 'gemini'), gauge('claude', acct, 'cg')));
  }
  const blank = $('#transcript > .blank');
  if (blank && !!$('.note.wall', blank) === !!S.accounts.length) (cur() ? blank.replaceWith(blankFor(cur())) : renderBlank());
  renderMeter();
  const probed = Math.max(0, ...S.accounts.map(x => x.probed || 0));
  $('#relay-hint').textContent = probed ? 'probed ' + ago(probed) : '';
  if (S.drawer && S.tab === 'accounts') renderPane();
}
let acctTimer;
function refreshAccounts() {
  clearTimeout(acctTimer);
  acctTimer = setTimeout(() => api('GET', 'accounts').then(a => { S.accounts = a; renderRelay(); }).catch(() => {}), 400);
}

/* ---------- header, goal bar, composer state ---------- */
const STATUS = {
  running: 'working', approval: 'needs your approval', waiting: 'waiting for an account', checking: 'running check', verifying: 'verifier reviewing',
};
function renderHeader() {
  const m = cur();
  const busy = m && m.status !== 'idle';
  $('#title').textContent = m ? (m.title || 'Untitled') : 'antiquota';
  $('#cwd').textContent = !m ? '' : m.worktree ? '⎇ ' + m.worktree.branch
    : '\u200e' + m.cwd.replace(/^\/Users\/[^/]+/, '~') + '\u200e';
  $('#cwd').title = m ? (m.worktree ? `Worktree of ${m.worktree.repo} (from ${m.worktree.base}) in ${m.cwd}` : m.cwd) : '';
  const st = $('#status');
  st.hidden = !busy;
  if (busy) {
    st.className = 'status ' + m.status;
    st.textContent = m.status === 'sleeping' ? 'sleeping until ' + clock(m.status_until) : (STATUS[m.status] || m.status);
  }
  $('#tokens').textContent = m && (m.usage.input || m.usage.output) ? `${fmtK(m.usage.input)} in · ${fmtK(m.usage.output)} out` : '';
  if (m) {
    setPicker($('#model'), $('#effort'), m.model);
    $('#mode').value = m.mode;
  }
  const q = m ? m.queue.length : 0;
  $('#queued').hidden = !q;
  $('#queued').textContent = q + ' queued';
  $('#goal-btn').classList.toggle('on', !!(m && m.goal && m.goal.state === 'active'));
  document.title = (busy ? '● ' : '') + (m ? (m.title || 'Untitled') + ' · ' : '') + 'antiquota';
  renderGoalbar();
  renderJobbar();
  renderPlanbar();
  renderMeter();
  syncSend();
}
function syncSend() {
  const m = cur(), b = $('#send');
  const busy = m && m.status !== 'idle';
  const text = $('#input').value.trim() || (ATT.length ? 'x' : '');
  b.classList.toggle('stop', !!busy && !text);
  b.textContent = busy ? (text ? 'Queue' : 'Stop') : 'Send';
  b.title = busy ? (text ? 'Queue after this turn (Enter)' : 'Stop (Esc)') : 'Send (Enter)';
  b.disabled = !busy && !text;
  const raw = $('#input').value, mode = raw.startsWith('!') ? 'shell' : raw.startsWith('#') ? 'note' : '';
  $('#composer').classList.toggle('shell', mode === 'shell');
  $('#composer').classList.toggle('note', mode === 'note');
  const hint = $('#mode-hint');
  hint.hidden = !mode;
  hint.textContent = mode === 'shell' ? (S.shells ? 'running…' : 'shell · output goes to agy with your next message')
    : mode === 'note' ? 'note · appended to this project\'s GEMINI.md' : '';
  if (mode) { b.textContent = mode === 'shell' ? 'Run' : 'Save'; b.classList.remove('stop'); b.disabled = raw.trim().length < 2; }
}
function renderGoalbar() {
  const m = cur(), bar = $('#goalbar');
  const g = m && m.goal;
  bar.hidden = !g;
  if (!g) return;
  const meta = [
    g.state === 'active' ? `round ${g.round}/${g.max_rounds}` : g.state,
    g.check && `check: ${g.check}`,
    g.verifier && `verifier: ${g.verifier_model || m.model}`,
  ].filter(Boolean).join('  ·  ');
  bar.replaceChildren(
    h('span', { class: 'g' }, '◎ goal'),
    h('div', {}, h('div', { class: 'obj', title: g.objective }, g.objective), h('div', { class: 'gm' }, meta)),
    h('div', { class: 'acts' },
      g.state === 'active'
        ? h('button', { class: 'btn small', onclick: () => api('POST', `sessions/${m.id}/stop`).catch(fail) }, 'Pause')
        : g.state !== 'met' && h('button', { class: 'btn small', onclick: () => api('POST', `sessions/${m.id}/goal/resume`).catch(fail) }, 'Resume'),
      h('button', { class: 'btn small', onclick: openGoal }, 'Edit'),
      h('button', { class: 'btn small', title: 'Remove goal', onclick: () => api('DELETE', `sessions/${m.id}/goal`).catch(fail) }, '✕')));
}

/* Plan mode: once agy has proposed a plan, carry it out (on its own or with approvals) or revise it. */
const EXECUTE_PLAN = 'Go ahead: carry out the plan you just proposed, step by step. Check each step works, '
  + 'and finish with a short summary of what changed.';
function renderPlanbar() {
  const m = cur(), bar = $('#planbar');
  const evs = S.events || [];
  let lastEnd = -1, lastUser = -1;
  evs.forEach((e, i) => { if (e.t === 'turn_end') lastEnd = i; if (e.t === 'user') lastUser = i; });
  const ready = m && m.mode === 'plan' && m.status === 'idle' && lastEnd > lastUser && evs[lastEnd].status === 'SUCCESS';
  bar.hidden = !ready;
  if (!ready) return;
  const run = mode => api('PATCH', 'sessions/' + m.id, { mode })
    .then(() => api('POST', `sessions/${m.id}/send`, { text: EXECUTE_PLAN }))
    .then(() => toast(mode === 'accept-edits' ? 'Executing the plan' : 'Executing: edits and commands ask you first')).catch(fail);
  bar.replaceChildren(
    h('span', { class: 'g' }, '▤ plan ready'),
    h('span', { class: 'pm' }, 'agy only planned (plan mode: nothing was changed).'),
    h('div', { class: 'acts' },
      h('button', { class: 'primary btn small', onclick: () => run('accept-edits') }, 'Execute plan'),
      h('button', { class: 'btn small', title: 'Edits and unknown commands wait for your approval', onclick: () => run('default') }, 'Execute with approvals'),
      h('button', { class: 'btn small', onclick: () => { const t = $('#input'); t.placeholder = 'What should change in the plan?'; t.focus(); } }, 'Revise')));
}

/* Background jobs (training runs and the like) and the /loop schedule, above the composer. */
function renderJobbar() {
  const m = cur(), bar = $('#jobbar');
  const running = (m?.jobs || []).filter(j => !j.ended), lp = m?.loop;
  bar.hidden = !running.length && !lp;
  if (bar.hidden) return;
  bar.replaceChildren(
    ...running.map(j => h('div', { class: 'job' },
      h('span', { class: 'pulse', 'aria-hidden': 'true' }),
      h('code', { title: j.cmd }, j.cmd),
      h('span', { class: 'jm', 'data-since': j.started }, elapsed(j.started) + (j.every ? ` · check-in every ${Math.round(j.every / 60)}m` : '') + (j.by === 'agent' ? ' · started by agy' : '')),
      h('button', { class: 'btn small', onclick: () => showLog(m.id, j.id) }, 'Log'),
      h('button', { class: 'btn small', onclick: () => api('POST', `sessions/${m.id}/jobs/${j.id}/stop`).then(() => toast('Stopped')).catch(fail) }, 'Stop'))),
    lp && h('div', { class: 'job loop' },
      h('span', { class: 'g' }, '↻'),
      h('code', { title: lp.prompt }, lp.prompt),
      h('span', { class: 'jm' }, (lp.at ? `daily at ${lp.at}` : `every ${fmtEvery(lp.every)}`) + ` · next ${new Date(lp.next * 1000).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit' })}`),
      h('button', { class: 'btn small', onclick: () => api('DELETE', `sessions/${m.id}/loop`).then(() => toast(lp.at ? 'Schedule removed' : 'Loop stopped')).catch(fail) }, lp.at ? 'Remove' : 'Stop loop')));
}
const elapsed = t => { const s = Math.max(0, Date.now() / 1000 - t) | 0; return s < 3600 ? `${s / 60 | 0}m${String(s % 60).padStart(2, '0')}s` : `${s / 3600 | 0}h${String((s / 60 | 0) % 60).padStart(2, '0')}m`; };
const fmtEvery = s => s % 3600 === 0 ? s / 3600 + 'h' : s % 60 === 0 ? s / 60 + 'm' : s + 's';
setInterval(() => $$('#jobbar .jm[data-since]').forEach(n => { n.firstChild.textContent = n.textContent.replace(/^\S+/, elapsed(+n.dataset.since)); }), 1000);
async function showLog(sid, jid) {
  try {
    const j = await api('GET', `sessions/${sid}/jobs/${jid}/log`);
    showFile({ rel: `job · ${j.cmd}`, path: j.log, text: j.text || '(no output yet)' });
    const b = $('#viewer-body');
    b.scrollTop = b.scrollHeight;
  } catch (e) { fail(e); }
}
/* Approvals: agy asks before running a command or editing a file (PreToolUse hook). */
function approvalNode(ev) {
  const a = ev.args || {}, isCmd = ev.tool === 'run_command';
  const target = isCmd ? a.CommandLine : rel(a.TargetFile || a.File || '');
  if (ev.state !== 'pending') {
    const ok = ev.state === 'allowed';
    return h('div', { class: 'approval done ' + (ok ? 'ok' : 'no') },
      h('span', { class: 'k' }, ok ? '✓ allowed' : '✕ declined'),
      h('code', { title: target }, target),
      h('span', { class: 'jm' }, ok ? (ev.always ? (isCmd ? `always: ${ev.rule}` : 'edits allowed for this session') : 'once')
        : (ev.why || 'by you')));
  }
  const answer = (decision, always) => api('POST', `approvals/${ev.id}/answer`, { decision, always })
    .catch(e => toast(/not found/.test(e.message) ? 'That request expired (the engine restarted)' : e.message));
  const preview = isCmd ? h('pre', { class: 'cmd' }, a.CommandLine)
    : a.CodeContent ? h('pre', { class: 'out' }, String(a.CodeContent).split('\n').slice(0, 14).join('\n'))
    : a.ReplacementContent ? h('pre', { class: 'out' }, String(a.ReplacementContent).split('\n').slice(0, 14).join('\n'))
    : null;
  return h('div', { class: 'approval', role: 'group', 'aria-label': 'Approval needed' },
    h('div', { class: 'ah' }, h('span', { class: 'k' }, 'Approval needed'),
      h('span', {}, isCmd ? 'agy wants to run a command' : `agy wants to edit ${target}`),
      h('span', { class: 'jm' }, 'waits ' + Math.round((ev.deadline - ev.ts) / 60) + ' min')),
    preview,
    h('div', { class: 'acts' },
      h('button', { class: 'primary btn small', onclick: () => answer('allow', false) }, 'Allow once'),
      isCmd && ev.rule ? h('button', { class: 'btn small', title: 'Added to antiquota\'s allow rules for every session', onclick: () => answer('allow', true) }, `Always allow ${ev.rule}`)
        : !isCmd ? h('button', { class: 'btn small', title: 'Switches this session to accept edits', onclick: () => answer('allow', true) }, 'Allow all edits in this session') : null,
      h('button', { class: 'btn small', onclick: () => answer('deny', false) }, 'Decline')));
}
function jobNode(ev) {
  const j = ev.job, end = ev.phase === 'ended';
  const status = !end ? 'started' : j.stopped ? 'stopped' : j.code === 0 ? 'exit 0' : j.code == null ? 'finished' : `exit ${j.code}`;
  return h('div', { class: 'jobline' + (end && j.code !== 0 && j.code != null ? ' bad' : '') },
    h('span', { class: 'k' }, end ? '■ job' : '▶ job'),
    h('code', { title: j.cmd }, j.cmd),
    h('span', { class: 'jm' }, status + (end ? ' · ' + secs(j.ended - j.started) : j.by === 'agent' ? ' · by agy' : '')),
    h('button', { class: 'btn small', onclick: () => showLog(S.cur, j.id) }, 'Log'));
}

/* ---------- transcript ---------- */
function nearBottom() {
  const s = $('#scroll');
  return s.scrollHeight - s.scrollTop - s.clientHeight < 120;
}
function scrollEnd() {
  const s = $('#scroll');
  s.scrollTop = s.scrollHeight;
}
function needAccount() {
  return !S.accounts.length && h('div', { class: 'note wall' },
    h('span', { class: 'k' }, 'No account yet'),
    'antiquota signs in its own Google accounts, separate from your agy CLI. Add at least one to start.',
    h('div', { class: 'acts' }, h('button', { class: 'primary btn small', onclick: () => openAcct() }, 'Add Google account')));
}
function blankFor(m) {
  return h('div', { class: 'blank' },
    needAccount(),
    h('h2', {}, 'What should agy work on?'),
    h('p', {}, `Runs headless in ${m.cwd.replace(/^\/Users\/[^/]+/, '~')} on the first account with quota left. `
      + 'Goal mode keeps the agent going until it is done and your checks agree, passing the work between accounts as quota runs out.'),
    h('div', { class: 'row' }, h('button', { class: 'btn', onclick: openGoal }, '◎ Set a goal')),
    pickUp(m),
    h('div', { class: 'keys' },
      h('kbd', {}, 'Enter'), h('span', {}, 'send'),
      h('kbd', {}, '⇧ Enter'), h('span', {}, 'new line'),
      h('kbd', {}, '/'), h('span', {}, 'commands'),
      h('kbd', {}, 'Esc'), h('span', {}, 'stop the running turn'),
      h('kbd', {}, '⌘N'), h('span', {}, 'new session')));
}
/* A new session can start from an earlier one's handoff state: antiquota's own record of it. */
function pickUp(m) {
  const earlier = [...S.sessions.values()].filter(x => x.id !== m.id && x.cwd === m.cwd && x.turns > 0)
    .sort((x, y) => y.updated - x.updated).slice(0, 3);
  if (!earlier.length) return null;
  return h('div', { class: 'pickup' }, h('span', { class: 'cap' }, 'Pick up from an earlier session here:'),
    ...earlier.map(x => h('button', {
      class: 'btn small', title: 'Its handoff state (requests, work log, files, last reply) goes in front of your next message',
      onclick: () => api('POST', `sessions/${m.id}/inherit`, { from: x.id }).then(() => toast('Picks up from ' + (x.title || 'that session'))).catch(fail),
    }, x.title || 'Untitled', h('span', { class: 'when' }, ' · ' + ago(x.updated)))));
}
function renderBlank() {
  $('#transcript').replaceChildren(h('div', { class: 'blank' },
    needAccount(),
    h('h2', {}, 'Start a session'),
    h('p', {}, 'Pick a working folder and a model. Every account you sign in with antiquota shares one stack of skills, hooks and MCP servers.'),
    h('div', { class: 'row' }, h('button', { class: 'primary', onclick: openNew }, 'New session'))));
  renderHeader();
}

const WHO = { user: 'you', goal: 'goal', nudge: 'goal push', job: 'job', loop: 'loop' };
function userNode(ev) {
  // A goal prompt carries standing instructions; the objective is what matters to read.
  const text = ev.kind === 'goal' ? ev.text.split('\n\n')[0].replace(/^Objective:\s*/, '') : ev.text;
  return h('div', { class: 'msg-user ' + (ev.kind || 'user') + (ev.steered ? ' steered' : ''), title: ev.kind === 'goal' ? ev.text : null },
    h('span', { class: 'who' }, (WHO[ev.kind] || 'you') + (ev.steered ? ' · while it worked' : '')), text, attachmentChips(ev, S.cur));
}
function firstString(p) {
  for (const v of Object.values(p || {})) if (typeof v === 'string' && v.length < 300) return v;
  return '';
}
function toolInfo(ev) {
  const p = ev.params || {};
  const T = {
    run_command: ['Run', p.CommandLine, 'run'],
    view_file: ['Read', rel(p.AbsolutePath), 'read'],
    list_dir: ['List', rel(p.DirectoryPath || p.SearchPath), 'list'],
    grep_search: ['Search', p.Query, 'search'],
    find_by_name: ['Find', p.Pattern, 'search'],
    write_to_file: ['Write', rel(p.TargetFile), 'edit'],
    replace_file_content: ['Edit', rel(p.TargetFile), 'edit'],
    multi_replace_file_content: ['Edit', rel(p.TargetFile), 'edit'],
    sed_file: ['Edit', rel(p.TargetFile || p.File), 'edit'],
    read_url_content: ['Fetch', p.Url, 'web'],
    search_web: ['Web search', p.query || p.Query, 'web'],
    open_browser_url: ['Browse', p.Url, 'web'],
    call_mcp_tool: ['MCP', [p.ServerName, p.ToolName].filter(Boolean).join(' · '), 'mcp'],
  }[ev.name];
  return T || [ev.name.replace(/_/g, ' '), firstString(p), 'other'];
}
function diffStats(d) {
  let add = 0, del = 0;
  for (const l of d.split('\n')) {
    if (l.startsWith('+') && !l.startsWith('+++')) add++;
    else if (l.startsWith('-') && !l.startsWith('---')) del++;
  }
  return [add, del];
}
function diffNode(d) {
  return h('div', { class: 'out diff' }, d.replace(/\n$/, '').split('\n').map(l => h('div', {
    class: l.startsWith('@@') || l.startsWith('+++') || l.startsWith('---') ? 'h' : l.startsWith('+') ? 'a' : l.startsWith('-') ? 'd' : '',
  }, l || ' ')));
}
const PEEK_LINES = 4;
function peekOf(ev) {
  // A Claude-Code-style glimpse under the row: last lines of output, first changed lines of a diff.
  if (ev.state === 'running') return null;
  if (ev.diff) {
    const lines = ev.diff.split('\n').filter(l => /^[+-]/.test(l) && !/^(\+\+\+|---)/.test(l));
    if (!lines.length) return null;
    return { node: diffNode(lines.slice(0, 6).join('\n')), more: Math.max(0, lines.length - 6) };
  }
  if (ev.output && ev.name === 'run_command') {
    const lines = ev.output.replace(/\s+$/, '').split('\n');
    return { node: h('pre', { class: 'out' }, lines.slice(-PEEK_LINES).join('\n')), more: Math.max(0, lines.length - PEEK_LINES) };
  }
  return null;
}
function fullBody(ev) {
  const body = h('div', { class: 'body' });
  if (ev.diff) body.append(diffNode(ev.diff));
  else if (ev.output) body.append(h('pre', { class: 'out' }, ev.output));
  if (ev.error && ev.error.split('\n').length > 2) body.append(h('pre', { class: 'out' }, ev.error));
  if (!ev.diff && !ev.output) body.append(h('pre', { class: 'out' }, JSON.stringify(ev.params, null, 2)));
  return body;
}
function toolNode(ev) {
  if (ev.output) ev = { ...ev, output: ev.output.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, '') };  // terminal colors
  const [verb, arg, ico] = toolInfo(ev);
  const stats = ev.diff ? diffStats(ev.diff) : null;
  const peek = peekOf(ev);
  const wrap = h('div', { class: 'tool ' + ev.state });
  const head = h('button', { class: 'tool-head', type: 'button', 'aria-expanded': 'false' },
    h('span', { class: 'ico', html: ICON[ico] }),
    h('span', { class: 'verb' }, verb),
    h('span', { class: 'arg', title: arg || '' }, arg || ''),
    h('span', { class: 'meta' },
      stats && stats[0] > 0 && h('span', { class: 'stat add' }, '+' + stats[0]),
      stats && stats[1] > 0 && h('span', { class: 'stat del' }, '−' + stats[1]),
      ev.state === 'running' ? 'running' : secs(ev.secs),
      h('span', { class: 'chev', 'aria-hidden': 'true' }, '›')));
  wrap.append(head);
  if (ev.error) wrap.append(h('div', { class: 'err' }, ev.error.split('\n').slice(0, 2).join('\n')));
  let peekNode = null;
  if (peek) {
    peekNode = h('div', { class: 'peek', onclick: () => toggle(true) }, peek.node,
      peek.more ? h('div', { class: 'more' }, `… ${peek.more} more line${peek.more > 1 ? 's' : ''}, click to expand`) : null);
    wrap.append(peekNode);
  }
  let body = null;
  function toggle(open = !wrap.classList.contains('open')) {
    wrap.classList.toggle('open', open);
    head.setAttribute('aria-expanded', String(open));
    if (open && !body) { body = fullBody(ev); wrap.append(body); }
    if (body) body.hidden = !open;
  }
  head.addEventListener('click', () => toggle());
  wrap.toggle = toggle;
  return wrap;
}
function thinkingNode(ev) {
  const d = h('details', { class: 'thinking' },
    h('summary', {}, h('span', { class: 'spark' }, '✻'), 'Thought',
      ev.tokens ? h('span', { class: 'tk' }, `${fmtK(ev.tokens)} tokens`) : null),
    h('div', { class: 'say think-body', html: md(ev.text) }));
  if (S.detail === 'detailed' && ev.text.length < 1200) d.open = true;
  return d;
}
function ruleFor(cmd) {
  const word = (cmd || '').trim().split(/\s+/)[0];
  return word ? `command(${word})` : null;
}
async function allow(rules, retry) {
  const have = (await api('GET', 'permissions')).allow;
  await api('PUT', 'permissions', { allow: [...have, ...rules] });
  toast('Allowed for every account: ' + rules.join(', '));
  if (retry) await api('POST', `sessions/${S.cur}/send`, { text: `Permission granted (${rules.join(', ')}). Retry what was denied and continue.` });
}
function noticeNode(ev) {
  const d = ev.data || {};
  switch (ev.kind) {
    case 'handoff':
      return h('div', { class: 'handoff' }, h('span', {}, '⇢ ' + ev.text), h('span', { class: 'fill' }));
    case 'wall':
      return h('div', { class: 'note wall' }, h('span', { class: 'k' }, 'Out of quota'), ev.text);
    case 'noaccount':
      return h('div', { class: 'note wall' }, h('span', { class: 'k' }, 'No account'), ev.text,
        h('div', { class: 'acts' }, h('button', { class: 'primary btn small', onclick: () => openAcct() }, 'Add Google account')));
    case 'goal':
      return h('div', { class: 'goalline' + (d.met ? ' met' : '') }, h('span', { class: 'g' }, d.met ? '✓' : '◎'), ev.text);
    case 'check': {
      const n = h('div', { class: 'verdict ' + (d.passed ? 'pass' : 'fail') },
        h('span', { class: 'k' }, d.passed ? 'Check passed' : 'Check failed'), ev.text);
      if (d.output) n.append(h('details', {}, h('summary', {}, 'output'), h('pre', { class: 'out' }, d.output)));
      return n;
    }
    case 'verdict': {
      const cls = d.done === true ? 'pass' : d.done === false ? 'fail' : '';
      return h('div', { class: 'verdict ' + cls },
        h('span', { class: 'k' }, d.done === true ? 'Verifier · done' : d.done === false ? 'Verifier · not met' : 'Verifier'),
        ev.text,
        (d.missing || []).length > 0 && h('ul', {}, d.missing.map(x => h('li', {}, x))),
        d.slot && h('div', { class: 'turn-end' }, `${d.slot} · ${d.model}`));
    }
    case 'denied': {
      const rules = [...new Set((d.commands || []).map(ruleFor).filter(Boolean))];
      const names = (d.actions || []).map(x => x.display_name || x.action);
      return h('div', { class: 'note denied' },
        h('span', { class: 'k' }, 'Permission denied'),
        ev.text,
        (d.commands || []).length > 0 && h('ul', {}, d.commands.map(c => h('li', {}, h('code', { class: 'path' }, c)))),
        !(d.commands || []).length && names.length > 0 && h('div', {}, 'Denied: ' + names.join(', ')),
        h('div', { class: 'acts' },
          rules.length > 0 && h('button', { class: 'btn small', onclick: () => allow(rules, false).catch(fail) }, 'Allow ' + rules.join(', ')),
          rules.length > 0 && h('button', { class: 'primary btn small', onclick: () => allow(rules, true).catch(fail) }, 'Allow & retry'),
          !rules.length && h('button', {
            class: 'btn small',
            onclick: () => api('PATCH', 'sessions/' + S.cur, { mode: 'accept-edits' })
              .then(() => api('POST', `sessions/${S.cur}/send`, { text: 'Edits are now allowed. Retry what was denied and continue.' })).catch(fail),
          }, 'Switch to accept edits & retry')));
    }
    case 'error':
      return h('div', { class: 'note error' }, h('span', { class: 'k' }, 'Error'), h('pre', {}, ev.text));
    case 'rewind':
      return h('div', { class: 'note' }, h('span', { class: 'k' }, '↶ Rewind'), ev.text,
        h('ul', {}, (d.files || []).map(f => h('li', {}, h('code', { class: 'path' }, f),
          (d.outside || []).includes(f) ? ' (it had changed after the agent\'s last edit)' : ''))));
    default:
      return h('div', { class: 'note' }, ev.text);
  }
}
function turnEnd(ev) {
  const u = ev.usage || {};
  const good = ev.status === 'SUCCESS';
  return h('div', { class: 'turn-end' },
    h('span', { class: good ? '' : 'bad' }, good ? '✓' : '✕ ' + ev.status),
    h('span', {}, secs(ev.secs)),
    u.total_tokens ? h('span', {}, `${fmtK(u.input_tokens)} in · ${fmtK(u.output_tokens)} out`) : null,
    h('span', {}, ev.slot));
}
function paint(st) {
  if (st.queued) return;
  st.queued = true;
  requestAnimationFrame(() => {
    st.queued = false;
    st.node.innerHTML = md(st.text);
  });
}
async function rewindTo(turn) {
  const m = cur();
  if (!m || !turn) return toast('Which turn? /rewind <turn number>');
  if (!await ask(`Every file the agent changed in turn ${turn} and later goes back to how it was before turn ${turn}.`,
    { title: `Rewind to before turn ${turn}?`, ok: 'Rewind' })) return;
  try {
    const r = await api('POST', `sessions/${m.id}/rewind`, { turn });
    toast(`Rewound ${r.files.length} file${r.files.length > 1 ? 's' : ''}`);
  } catch (e) { fail(e); }
}
function copyButton(getText) {
  return h('button', {
    class: 'copy-msg', type: 'button', title: 'Copy', 'aria-label': 'Copy message',
    onclick: e => { e.stopPropagation(); copyText(getText()).then(() => toast('Copied')); },
  }, '⧉');
}
function renderEvent(ev, live) {
  const tr = $('#transcript');
  const stick = nearBottom();
  $('.blank', tr)?.remove();
  switch (ev.t) {
    case 'user':
      tr.append(userNode(ev));
      break;
    case 'turn_start':
      $$('.turn.live', tr).forEach(n => n.classList.remove('live'));
      tr.append(h('div', { class: 'turn' + (live ? ' live' : '') },
        h('span', { class: 'tn' }, '#' + ev.turn),
        h('span', { class: 'acc', title: ev.slot }, ev.slot), h('span', {}, ev.model), h('span', { class: 'fill' }),
        h('button', { class: 'rewind', type: 'button', title: `Put files back as they were before turn ${ev.turn}`,
          onclick: () => rewindTo(ev.turn) }, '↶ rewind'),
        h('span', {}, clock(ev.ts))));
      break;
    case 'delta': {
      const key = ev.turn + ':' + ev.step;
      let st = S.live.get(key);
      if (!st) {
        st = { text: '', node: h('div', { class: 'say live' }) };
        S.live.set(key, st);
        S.nodes.set(key, st.node);
        tr.append(st.node);
      }
      st.text += ev.text;
      paint(st);
      break;
    }
    case 'assistant': {
      const key = ev.turn + ':' + ev.step;
      let node = S.nodes.get(key);
      if (!node) { node = h('div', { class: 'say' }); S.nodes.set(key, node); tr.append(node); }
      const st = S.live.get(key);
      if (st) { st.text = ev.text; st.queued = true; S.live.delete(key); }
      node.classList.remove('live');
      node.innerHTML = md(ev.text);
      addCopy(node);
      node.append(copyButton(() => ev.text));
      break;
    }
    case 'tool': {
      const key = 't' + ev.turn + ':' + ev.step;
      const node = toolNode(ev), old = S.nodes.get(key);
      if (old) { old.replaceWith(node); if (old.classList.contains('open')) node.toggle(true); } else tr.append(node);
      S.nodes.set(key, node);
      break;
    }
    case 'thinking': {
      const key = 'th' + ev.turn + ':' + ev.step;
      const node = thinkingNode(ev), reply = S.nodes.get(ev.turn + ':' + ev.step), old = S.nodes.get(key);
      if (old) old.replaceWith(node);
      else if (reply && reply.parentNode === tr) tr.insertBefore(node, reply);
      else tr.append(node);
      S.nodes.set(key, node);
      break;
    }
    case 'turn_end':
      if (live && ev.status !== 'quota') notify(`${cur()?.title || 'antiquota'}: turn ${ev.turn} ${ev.status === 'SUCCESS' ? 'done' : ev.status}`, `${ev.slot} · ${secs(ev.secs)}`);
      $$('.turn.live', tr).forEach(n => n.classList.remove('live'));
      for (const [k, st] of S.live) { st.node.classList.remove('live'); S.live.delete(k); }
      tr.append(turnEnd(ev));
      break;
    case 'shell':
      tr.append(shellNode(ev));
      break;
    case 'approval': {
      const key = 'ap' + ev.id, node = approvalNode(ev), old = S.nodes.get(key);
      if (old) old.replaceWith(node); else tr.append(node);
      S.nodes.set(key, node);
      if (live && ev.state === 'pending') {
        notify('Approval needed', (ev.args || {}).CommandLine || (ev.args || {}).TargetFile || ev.tool);
        if (NATIVE) NATIVE.window.getCurrentWindow().requestUserAttention(1).catch(() => {});
      }
      break;
    }
    case 'job':
      tr.append(jobNode(ev));
      if (live && ev.phase === 'ended') notify(`Job finished: ${ev.job.cmd}`, cur()?.title || '');
      break;
    case 'notice':
      if (live && ev.kind === 'goal' && (ev.data || {}).met) notify('Goal met', cur()?.title || '');
      if (live && ev.kind === 'wall') notify('Out of quota', ev.text);
      tr.append(noticeNode(ev));
      break;
  }
  if (stick) scrollEnd();
}

async function openSession(sid) {
  if (S.cur !== sid && ATT.length) { ATT.length = 0; renderTray(); }
  S.cur = sid;
  history.replaceState(null, '', '#' + sid);
  S.nodes.clear();
  S.live.clear();
  renderSessions();
  const { meta, events } = await api('GET', 'sessions/' + sid);
  if (S.cur !== sid) return;
  S.sessions.set(sid, meta);
  const tr = $('#transcript');
  tr.replaceChildren();
  if (!events.length) tr.append(blankFor(meta));
  S.events = events;
  HIST.list = events.filter(e => e.t === 'user' && e.kind === 'user').map(e => e.text);
  HIST.pos = -1;
  for (const ev of events) renderEvent(ev, false);
  renderSessions();
  renderHeader();
  scrollEnd();
  if (S.drawer && S.tab !== 'accounts') renderPane();  // changes and files follow the open session
  $('#side').classList.remove('open');
  $('#input').focus();
}

/* ---------- live events ---------- */
const KEPT = new Set(['user', 'turn_start', 'turn_end', 'assistant', 'notice', 'approval', 'job', 'shell']);
function handle(m) {
  if (m.t === 'accounts') {
    S.accounts = m.accounts;
    renderRelay();
    if (!S.models.length) api('GET', 'state').then(st => { S.models = st.models; if (S.models.length) { fillModels(); renderHeader(); } }).catch(() => {});
    return;
  }
  if (m.t === 'turn_start' || m.t === 'turn_end') refreshAccounts();
  if (m.t === 'meta') {
    S.sessions.set(m.sid, m.meta);
    renderSessions();
    if (m.sid === S.cur) renderHeader();
    return;
  }
  if (m.t === 'deleted') {
    S.sessions.delete(m.sid);
    renderSessions();
    if (m.sid === S.cur) {
      S.cur = null;
      const next = [...S.sessions.values()].sort((a, b) => b.updated - a.updated)[0];
      next ? openSession(next.id) : renderBlank();
    }
    return;
  }
  if (m.sid === S.cur) {
    if (m.t === 'tool' && m.state !== 'running') {
      S.events.push(m);
      if (S.drawer && S.tab === 'changes') renderPane();
    } else if (KEPT.has(m.t)) S.events.push(m);
    renderEvent(m, true);
    if (m.t === 'turn_end' || m.t === 'user') renderPlanbar();
  }
}
function connect() {
  const es = new EventSource('/api/events?token=' + encodeURIComponent(TOKEN));
  let dropped = false;
  es.onmessage = e => handle(JSON.parse(e.data));
  es.onerror = () => { dropped = true; };
  es.onopen = () => { if (dropped) { dropped = false; load(); } };
}

/* ---------- composer ---------- */
function ensureOption(sel, value) {
  if (value && ![...sel.options].some(o => o.value === value)) sel.append(h('option', { value }, value));
}
/* Models as agy lists them: "Gemini 3.8 Flash (High)" is family "Gemini 3.8 Flash", effort
   "high". Gemini families come in effort variants; Claude and GPT-OSS have one fixed model. */
const POOL = id => /^(claude|gpt)/.test(id) ? 'cg' : 'gemini';
const POOL_LABEL = { gemini: 'Gemini quota', cg: 'Claude / GPT quota (small limits)' };
function families(extraId) {
  const list = [...S.models];
  if (extraId && !list.some(m => m.id === extraId)) list.push({ id: extraId, label: extraId });
  const fams = new Map();
  for (const m of list) {
    const mm = m.label.match(/^(.*?)\s*\(([^)]+)\)\s*$/);
    const name = mm ? mm[1] : m.label;
    if (!fams.has(name)) fams.set(name, { name, pool: POOL(m.id), variants: [] });
    fams.get(name).variants.push({ id: m.id, effort: (mm ? mm[2] : '').toLowerCase() });
  }
  return [...fams.values()];
}
const familyOf = (id, fams = families(id)) => fams.find(f => f.variants.some(v => v.id === id));
const effortOf = id => familyOf(id)?.variants.find(v => v.id === id)?.effort || '';
function groupedOptions(fams, value, text) {
  return ['gemini', 'cg'].map(pool => {
    const g = fams.filter(f => f.pool === pool);
    return g.length ? h('optgroup', { label: POOL_LABEL[pool] }, g.map(f => h('option', { value: value(f) }, text(f)))) : null;
  }).filter(Boolean);
}
/* A model picker is two selects: family (grouped by quota pool), then that family's efforts. */
function setPicker(famSel, effSel, id) {
  const fams = families(id), fam = familyOf(id, fams);
  famSel.replaceChildren(...groupedOptions(fams, f => f.name, f => f.name));
  famSel.value = fam.name;
  effSel.replaceChildren(...fam.variants.map(v => h('option', { value: v.id }, v.effort || 'default')));
  effSel.value = id;
  effSel.disabled = fam.variants.length < 2;
  effSel.title = fam.variants.length < 2 ? 'This model has one fixed effort' : '';
}
function pickFamily(famSel, effSel, prevId) {
  const fam = families(prevId).find(f => f.name === famSel.value);
  const want = effortOf(prevId);
  const v = fam.variants.find(x => x.effort === want) || fam.variants.find(x => x.effort === 'high') || fam.variants[0];
  setPicker(famSel, effSel, v.id);
  return v.id;
}
function setModelSelect(sel, id) {  // one concrete model id, e.g. the goal verifier
  const fams = families(id);
  sel.replaceChildren(...['gemini', 'cg'].map(pool => {
    const g = fams.filter(f => f.pool === pool);
    return g.length ? h('optgroup', { label: POOL_LABEL[pool] },
      g.flatMap(f => f.variants.map(v => h('option', { value: v.id }, f.name + (v.effort ? ` · ${v.effort}` : ''))))) : null;
  }).filter(Boolean));
  sel.value = id;
}
function fillModels() {
  const m = cur();
  setPicker($('#model'), $('#effort'), m ? m.model : S.defaults.model);
}
function autosize() {
  const t = $('#input');
  t.style.height = 'auto';
  t.style.height = Math.min(t.scrollHeight, innerHeight * .4) + 'px';
}
async function submit() {
  const t = $('#input'), text = t.value.trim(), m = cur();
  const ready = ATT.filter(f => !f.uploading && f.path);
  if (ATT.some(f => f.uploading)) return toast('Still uploading…');
  if (!text && !ready.length) { if (m && m.status !== 'idle') stop(); return; }
  if (text.startsWith('/')) { t.value = ''; autosize(); closeSlash(); return slash(text); }
  if (!m) return openNew(text);
  if (/^[!#]/.test(text) && text.length > 1) return local(m, t, text);
  HIST.list.push(text);
  HIST.pos = -1;
  if (!NATIVE && 'Notification' in window && Notification.permission === 'default') Notification.requestPermission().catch(() => {});
  t.value = '';
  autosize();
  syncSend();
  try {
    const r = await api('POST', `sessions/${m.id}/send`, { text, attachments: ready.map(f => f.path) });
    ATT.length = 0;
    renderTray();
    if (r.result === 'queued') toast(ready.length ? 'Queued after the current turn' : 'Sent: agy reads it at its next step');
  } catch (e) { t.value = text; fail(e); }
}
/* `!cmd` runs in the project folder and agy sees the output with the next message; `# note`
   goes into the project's GEMINI.md. Both are Claude Code habits. */
async function local(m, t, text) {
  HIST.list.push(text);
  HIST.pos = -1;
  t.value = '';
  autosize();
  syncSend();
  const body = text.slice(1).trim();
  try {
    if (text[0] === '#') {
      const r = await api('POST', `sessions/${m.id}/note`, { text: body });
      return toast(`Saved to ${r.file}; agy reads it in every conversation here`);
    }
    S.shells = (S.shells || 0) + 1;
    syncSend();
    await api('POST', `sessions/${m.id}/shell`, { cmd: body });
  } catch (e) { t.value = text; fail(e); }
  finally { if (text[0] === '!') { S.shells--; syncSend(); } }
}
function shellNode(ev) {
  const node = toolNode({ name: 'run_command', params: { CommandLine: ev.cmd }, output: ev.output, secs: ev.secs,
    state: ev.code === 0 ? 'done' : 'error', error: ev.code === 0 ? '' : ev.code === null ? 'timed out' : `exit ${ev.code}` });
  node.classList.add('mine');
  $('.verb', node).textContent = 'You ran';
  return node;
}
function stop() {
  if (S.cur) api('POST', `sessions/${S.cur}/stop`).then(() => toast('Stopped')).catch(fail);
}
const patch = fields => S.cur && api('PATCH', 'sessions/' + S.cur, fields).catch(fail);

/* ---------- slash commands ---------- */
const SLASH = [
  ['/help', 'keyboard shortcuts and every command'],
  ['/goal', 'set a goal; empty opens the goal panel'],
  ['/clear', 'start a fresh agy conversation (the transcript stays)'],
  ['/compact', 'fresh conversation that starts from a summary of this one'],
  ['/rewind', 'put files back as they were before a turn: /rewind <turn>'],
  ['/export', 'download this transcript as markdown'],
  ['/init', 'have agy write a GEMINI.md that explains this project'],
  ['/bg', 'run a command in the background; agy gets its log when it exits: /bg <command>'],
  ['/loop', 'send a prompt now and every interval while idle: /loop 15m <prompt> · /loop off'],
  ['/schedule', 'send a prompt every day at a time: /schedule 03:00 <prompt> · /schedule off'],
  ['/memory', 'show this project\'s GEMINI.md (add lines with # note)'],
  ['/state', 'show the handoff state antiquota keeps for this session'],
  ['/stop', 'stop the running turn, pause the goal'],
  ['/resume', 'resume a paused goal'],
  ['/cleargoal', 'remove the goal'],
  ['/new', 'new session'],
  ['/model', 'switch model: /model <id>'],
  ['/effort', 'effort for Gemini models: low · medium · high'],
  ['/mode', 'permissions: default · accept-edits · plan · bypass'],
  ['/rename', 'rename this session'],
  ['/quota', 'probe quota on every account now'],
  ['/accounts', 'show accounts'],
  ['/login', 'add a Google account'],
  ['/settings', 'rules, skills, hooks, MCP, plugins, permissions'],
  ['/stack', 'show skills, hooks, MCP servers'],
  ['/permissions', 'edit antiquota allow rules'],
  ['/delete', 'delete this session'],
];
/* One popup for both slash commands and @-mentions. Items are [label, detail, apply()]. */
let slashIdx = 0, menuItems = [], mentionSeq = 0;
function showMenu(items) {
  menuItems = items;
  const box = $('#slash');
  if (!items.length) return closeSlash();
  slashIdx = Math.min(slashIdx, items.length - 1);
  box.replaceChildren(...items.map(([label, detail, apply], i) => h('button', {
    type: 'button', class: i === slashIdx ? 'on' : '', role: 'option',
    onmousedown: e => { e.preventDefault(); apply(); closeSlash(); syncSend(); },
  }, h('b', {}, label), h('span', {}, detail))));
  box.hidden = false;
}
function openSlashMenu() {
  const t = $('#input'), v = t.value;
  if (v.startsWith('/') && !/\s/.test(v)) {
    return showMenu(SLASH.filter(([c]) => c.startsWith(v.toLowerCase()))
      .map(([c, d]) => [c, d, () => { t.value = c + ' '; t.focus(); }]));
  }
  const before = v.slice(0, t.selectionStart), m = before.match(/(^|\s)@([\w./~-]*)$/);
  if (!m || !cur()) return closeSlash();
  const q = m[2], seq = ++mentionSeq;
  api('GET', `sessions/${S.cur}/files?q=${encodeURIComponent(q)}`).then(files => {
    if (seq !== mentionSeq) return;  // a newer keystroke already asked
    showMenu(files.map(f => [f, 'file', () => {
      const start = before.length - q.length;
      t.value = v.slice(0, start) + f + ' ' + v.slice(t.selectionStart);
      t.selectionStart = t.selectionEnd = start + f.length + 1;
      t.focus();
    }]));
  }).catch(() => {});
}
function closeSlash() { $('#slash').hidden = true; slashIdx = 0; menuItems = []; }

/* ---------- file viewer: agy's file:// links open here ---------- */
async function openFile(path, lines) {
  const m = cur();
  if (!m) return;
  try { showFile(await api('GET', `sessions/${m.id}/file?path=${encodeURIComponent(path)}`), lines); } catch (e) { fail(e); }
}
function showFile(f, [from, to] = [0, 0]) {
  to = to || from;
  const dlg = $('#viewer');
  $('#viewer-path').textContent = f.rel;
  $('#viewer-path').title = f.path;
  $('#viewer-copy').onclick = () => copyText(f.text).then(() => toast('Copied'));
  const lines = f.text.replace(/\n$/, '').split('\n');
  const body = $('#viewer-body');
  body.replaceChildren(h('pre', {}, ...lines.map((l, i) => h('div', { class: 'ln' + (i + 1 >= from && i + 1 <= to ? ' hit' : '') },
    h('span', { class: 'no' }, String(i + 1)), h('span', {}, l || ' ')))));
  dlg.showModal();
  $('#viewer-close').focus();
  const hit = $('.hit', body);
  if (hit) hit.scrollIntoView({ block: 'center' });
  else body.scrollTop = 0;
}

/* ---------- attachments: + button, drag and drop, pasted screenshots ---------- */
const attUrl = (sid, name) => `/api/sessions/${sid}/attachments/${encodeURIComponent(name)}?token=${encodeURIComponent(TOKEN)}`;
function renderTray() {
  const tray = $('#tray');
  tray.hidden = !ATT.length;
  tray.replaceChildren(...ATT.map((f, i) => h('div', { class: 'chip-att' + (f.uploading ? ' up' : ''), title: f.name },
    f.image && f.url ? h('img', { src: f.url, alt: '' }) : h('span', { class: 'fi' }, (f.name.split('.').pop() || 'file').slice(0, 4)),
    h('span', { class: 'fn' }, f.name),
    h('span', { class: 'fs' }, f.uploading ? 'uploading…' : fmtSize(f.size)),
    h('button', { type: 'button', title: 'Remove', 'aria-label': 'Remove ' + f.name, onclick: () => { ATT.splice(i, 1); renderTray(); syncSend(); } }, '×'))));
}
const fmtSize = n => n > 1e6 ? (n / 1e6).toFixed(1) + ' MB' : n > 1e3 ? Math.round(n / 1e3) + ' KB' : n + ' B';
async function attachFiles(files) {
  const m = cur();
  if (!m) { toast('Open or create a session first'); return; }
  for (const file of files) {
    if (file.size > 25e6) { toast(`${file.name} is over 25 MB`); continue; }
    const slot = { name: file.name || 'pasted.png', size: file.size, uploading: true,
      image: /^image\//.test(file.type), url: /^image\//.test(file.type) ? URL.createObjectURL(file) : null };
    ATT.push(slot);
    renderTray();
    try {
      const r = await fetch(`/api/sessions/${m.id}/attach`, {
        method: 'POST', body: file,
        headers: { 'X-Antiquota-Token': TOKEN, 'X-Filename': encodeURIComponent(slot.name) },
      });
      const j = await r.json();
      if (!r.ok) throw new Error(j.error || r.statusText);
      Object.assign(slot, j, { uploading: false });
    } catch (e) {
      ATT.splice(ATT.indexOf(slot), 1);
      fail(e);
    }
    renderTray();
    syncSend();
  }
  $('#input').focus();
}
function attachmentChips(ev, sid) {
  if (!ev.attachments || !ev.attachments.length) return null;
  return h('div', { class: 'msg-atts' }, ev.attachments.map(f => f.image
    ? h('a', { href: attUrl(sid, f.name), target: '_blank', rel: 'noopener', title: f.name }, h('img', { src: attUrl(sid, f.name), alt: f.name }))
    : h('span', { class: 'chip-att small' }, h('span', { class: 'fi' }, (f.name.split('.').pop() || 'file').slice(0, 4)), h('span', { class: 'fn' }, f.name))));
}

/* ---------- prompt history (↑ / ↓ in an empty or untouched composer) ---------- */
const HIST = { list: [], pos: -1, draft: '' };
function historyStep(dir) {
  const t = $('#input');
  if (!HIST.list.length) return false;
  if (HIST.pos === -1) HIST.draft = t.value;
  HIST.pos = Math.max(-1, Math.min(HIST.list.length - 1, HIST.pos + dir));
  t.value = HIST.pos === -1 ? HIST.draft : HIST.list[HIST.list.length - 1 - HIST.pos];
  autosize();
  syncSend();
  return true;
}

/* ---------- desktop notifications for long runs ---------- */
async function notify(title, body) {
  if (!document.hidden && document.hasFocus()) return;
  if (NATIVE) {
    try {
      const n = NATIVE.notification;
      if (!(await n.isPermissionGranted()) && (await n.requestPermission()) !== 'granted') return;
      n.sendNotification({ title, body });
    } catch {}
    return;
  }
  if (!('Notification' in window) || Notification.permission !== 'granted') return;
  try { new Notification(title, { body, tag: 'antiquota' }); } catch {}
}

/* ---------- export the transcript as markdown ---------- */
async function exportTranscript() {
  const m = cur();
  if (!m) return;
  const { events } = await api('GET', 'sessions/' + m.id);
  const out = [`# ${m.title || 'Untitled'}`, '', `${m.cwd} · ${m.model}`, ''];
  for (const e of events) {
    if (e.t === 'user') out.push(`## ${WHO[e.kind] || 'you'}`, '', e.text, '');
    else if (e.t === 'assistant') out.push(e.text, '');
    else if (e.t === 'thinking') out.push('<details><summary>Thought</summary>', '', e.text, '', '</details>', '');
    else if (e.t === 'tool') out.push(`- \`${e.name}\` ${toolInfo(e)[1] || ''}${e.state === 'error' ? ' (error)' : ''}`);
    else if (e.t === 'turn_start') out.push('', `---`, `*turn ${e.turn} · ${e.slot} · ${e.model}*`, '');
    else if (e.t === 'notice') out.push(`> **${e.kind}**: ${e.text}`, '');
  }
  await saveText((m.title || 'session').replace(/[^\w.-]+/g, '-').slice(0, 60) + '.md', out.join('\n'));
}

async function slash(text) {
  const [cmd, ...rest] = text.split(/\s+/);
  const arg = text.slice(cmd.length).trim();
  const m = cur();
  const need = () => { if (!m) throw new Error('Open a session first'); };
  try {
    switch (cmd.toLowerCase()) {
      case '/goal':
        need();
        if (!arg) return openGoal();
        await api('POST', `sessions/${m.id}/goal`, { objective: arg, verifier: true, verifier_model: S.defaults.verifier_model });
        return toast('Goal started');
      case '/stop': return stop();
      case '/help': return showShortcuts();
      case '/clear': need(); await api('POST', `sessions/${m.id}/clear`); return toast('Fresh conversation from the next message');
      case '/compact': need(); await api('POST', `sessions/${m.id}/compact`); return toast('Compacted');
      case '/rewind': return rewindTo(+arg || 0);
      case '/export': return exportTranscript();
      case '/init':
        need();
        return api('POST', `sessions/${m.id}/send`, { text: 'Study this project and write a concise GEMINI.md at its root for '
          + 'future agents: what it is, how it is laid out, how to build, run and test it, and the conventions to follow. '
          + 'If GEMINI.md or AGENTS.md already exists, improve it instead of starting over. Keep it under 80 lines.' + (arg ? '\n\n' + arg : '') });
      case '/bg':
        need();
        if (!arg) return toast('/bg <command>');
        await api('POST', `sessions/${m.id}/jobs`, { cmd: arg });
        return toast('Running in the background');
      case '/loop': {
        need();
        if (!arg || /^(off|stop)$/i.test(arg)) { await api('DELETE', `sessions/${m.id}/loop`); return toast('Loop stopped'); }
        const [every, ...p] = arg.split(/\s+/);
        await api('POST', `sessions/${m.id}/loop`, { every, prompt: p.join(' ') });
        return toast(`Every ${every}: ${p.join(' ').slice(0, 60)}`);
      }
      case '/state': {
        need();
        const r = await api('GET', `sessions/${m.id}/state`);
        return showFile({ rel: 'handoff state · ' + (m.title || m.id), path: 'STATE.md', text: r.text });
      }
      case '/schedule': {
        need();
        if (!arg || /^(off|stop)$/i.test(arg)) { await api('DELETE', `sessions/${m.id}/loop`); return toast('Schedule removed'); }
        const [at, ...p] = arg.split(/\s+/);
        const r = await api('POST', `sessions/${m.id}/loop`, { at, prompt: p.join(' ') });
        return toast(`Every day at ${at}; next ${new Date(r.next * 1000).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit' })}`);
      }
      case '/memory': {
        need();
        const f = await api('GET', `sessions/${m.id}/file?path=GEMINI.md`).catch(() => api('GET', `sessions/${m.id}/file?path=AGENTS.md`)).catch(() => null);
        return f ? showFile(f) : toast('No GEMINI.md yet: add a line with "# …" or run /init');
      }
      case '/resume': need(); return api('POST', `sessions/${m.id}/goal/resume`);
      case '/cleargoal': need(); return api('DELETE', `sessions/${m.id}/goal`);
      case '/new': return openNew();
      case '/model': need(); await patch({ model: arg }); return toast('Model: ' + arg);
      case '/effort': {
        need();
        const v = familyOf(m.model)?.variants.find(x => x.effort === arg.toLowerCase());
        if (!v) return toast(`${familyOf(m.model)?.name || m.model} has no effort "${arg}"`);
        await patch({ model: v.id });
        return toast('Model: ' + v.id);
      }
      case '/mode': need(); await patch({ mode: arg }); return toast('Mode: ' + arg);
      case '/rename': need(); return patch({ title: arg });
      case '/quota': await api('POST', 'accounts/probe'); return toast('Probing accounts…');
      case '/accounts': return showDrawer('accounts');
      case '/login': return openAcct();
      case '/stack': return showDrawer('stack');
      case '/skills': return openSettings('skills');
      case '/hooks': return openSettings('hooks');
      case '/mcp': return openSettings('mcp');
      case '/permissions': return openSettings('permissions');
      case '/settings': return openSettings();
      case '/delete':
        need();
        if (await ask('The transcript and its handoff state are deleted. Files in the project stay.',
          { title: `Delete "${m.title || 'Untitled'}"?`, ok: 'Delete', danger: true })) await api('DELETE', 'sessions/' + m.id);
        return;
      default:
        toast('Unknown command ' + cmd);
    }
  } catch (e) { fail(e); }
}

/* ---------- keyboard shortcuts and commands (⌘/ or /help) ---------- */
const KEYS = [['⌘N', 'new session'], ['⌘K', 'find a session or message'], ['⌘J', 'focus the message box'],
  ['⌘⇧[  ⌘⇧]', 'previous / next session'], ['⌘B', 'show or hide the side panel'], ['⌘.', 'stop the running turn'],
  ['⌘,', 'settings'], ['⌘/', 'this sheet'], ['Enter', 'send · ⇧Enter new line'], ['↑ ↓', 'earlier messages (empty box)'],
  ['Esc', 'stop the running turn'], ['@', 'mention a file'], ['!', 'run a shell command'], ['#', 'add a note to GEMINI.md']];
function showShortcuts() {
  const mac = /Mac/.test(navigator.platform), k = s => mac ? s : s.replace(/⌘/g, 'Ctrl+').replace(/⇧/g, 'Shift+');
  const dlg = h('dialog', { class: 'dlg keys-dlg', 'aria-label': 'Keyboard shortcuts' },
    h('form', { method: 'dialog' },
      h('h2', {}, 'Shortcuts and commands'),
      h('div', { class: 'keys-grid' },
        h('div', {}, h('h3', {}, 'Keys'), h('dl', {}, ...KEYS.flatMap(([a, b]) => [h('dt', {}, h('kbd', {}, k(a))), h('dd', {}, b)]))),
        h('div', {}, h('h3', {}, 'Commands'), h('dl', {}, ...SLASH.flatMap(([c, d]) => [h('dt', {}, h('code', {}, c)), h('dd', {}, d)])))),
      h('div', { class: 'actions' }, h('button', { class: 'primary' }, 'Close'))));
  dlg.addEventListener('close', () => dlg.remove());
  document.body.append(dlg);
  dlg.showModal();
}

/* ---------- files: browse the session folder ---------- */
async function renderTree(pane, m, path) {
  let t;
  try { t = await api('GET', `sessions/${m.id}/tree?path=${encodeURIComponent(path)}`); } catch (e) { return fail(e); }
  S.treePath = t.path; S.treeFor = m.id;
  const parts = t.path ? t.path.split('/') : [];
  const go = p => renderTree(pane, m, p);
  const mention = rel => { const i = $('#input'); i.value = (i.value.trimEnd() + ' @' + rel + ' ').trimStart(); i.focus(); autosize(); syncSend(); };
  const crumbs = h('div', { class: 'crumbs' },
    h('button', { class: 'crumb', onclick: () => go('') }, t.root.split(/[\\/]/).pop()),
    ...parts.flatMap((p, i) => ['/', h('button', { class: 'crumb', onclick: () => go(parts.slice(0, i + 1).join('/')) }, p)]));
  const rows = t.entries.map(e => {
    const rel = (t.path ? t.path + '/' : '') + e.name;
    const full = t.root + '/' + rel;
    return h('div', { class: 'frow' + (e.dim ? ' dim' : '') },
      h('button', { class: 'fname', title: rel, onclick: () => e.dir ? go(rel) : openFile(full, [0, 0]) },
        h('span', { class: 'fic', 'aria-hidden': 'true' }, e.dir ? '▸' : '·'), e.name + (e.dir ? '/' : '')),
      h('span', { class: 'fsz' }, e.dir ? '' : fmtSize(e.size)),
      h('span', { class: 'facts' },
        h('button', { class: 'btn small', title: 'Mention it in your message (@' + rel + ')', onclick: () => mention(rel) }, '@'),
        NATIVE && !e.dir && h('button', { class: 'btn small', title: 'Open in its default app', onclick: () => NATIVE.opener.openPath(full).catch(fail) }, 'Open'),
        NATIVE && h('button', { class: 'btn small', title: 'Show in Finder / Explorer', onclick: () => NATIVE.opener.revealItemInDir(full).catch(fail) }, '⌕')));
  });
  pane.replaceChildren(crumbs,
    t.path ? h('div', { class: 'frow' }, h('button', { class: 'fname', onclick: () => go(parts.slice(0, -1).join('/')) }, h('span', { class: 'fic' }, '↰'), '..')) : null,
    ...(rows.length ? rows : [h('p', { class: 'cap' }, 'Empty folder.')]));
}

/* ---------- git: review uncommitted work, commit, merge a worktree back ---------- */
async function renderGit(box, m) {
  let st;
  try { st = await api('GET', `sessions/${m.id}/git`); } catch { return; }
  if (!st || !st.root) { box.replaceChildren(); return; }
  const wt = st.worktree;
  const fileRow = (f, committed) => {
    const row = h('details', { class: 'change' },
      h('summary', {}, h('span', { class: 'code c-' + (f.code || 'M').replace('?', 'U') }, f.code === '??' ? 'new' : (f.code || '')),
        h('span', { class: 'cpath', title: f.path }, f.path),
        h('span', { class: 'meta' }, f.add ? h('span', { class: 'stat add' }, '+' + f.add) : null,
          f.del ? h('span', { class: 'stat del' }, '−' + f.del) : null,
          !committed && h('button', { class: 'btn small ghostish', title: 'Put this file back as it is in the last commit',
            onclick: async e => {
              e.preventDefault();
              if (await ask(f.code === '??' ? 'The new file is deleted.' : 'Its uncommitted changes are lost.', { title: `Discard ${f.path}?`, ok: 'Discard', danger: true }))
                api('POST', `sessions/${m.id}/git/discard`, { path: f.path }).then(() => renderGit(box, m)).catch(fail);
            } }, 'Discard'))));
    row.addEventListener('toggle', async () => {
      if (!row.open || row.children.length > 1) return;
      const r = await api('GET', `sessions/${m.id}/git/diff?path=${encodeURIComponent(f.path)}${committed ? '&committed=1' : ''}`).catch(fail);
      if (r) row.append(diffNode(r.diff || '(no textual change)'));
    });
    return row;
  };
  const msg = h('textarea', { class: 'commit-msg', rows: 2, placeholder: 'Commit message', spellcheck: 'true',
    'aria-label': 'Commit message' });
  msg.value = m.title || '';
  box.replaceChildren(...[
    h('div', { class: 'githead' },
      h('span', { class: 'branch' }, '⎇ ' + st.branch),
      wt ? h('span', { class: 'cap' }, `worktree from ${wt.base} · ${st.ahead} commit${st.ahead === 1 ? '' : 's'}` + (st.merged ? ` · merged into ${wt.base}` : ' ahead'))
        : h('span', { class: 'cap', title: st.root }, st.root.replace(/^\/Users\/[^/]+/, '~'))),
    h('h3', {}, st.files.length ? `Uncommitted · ${st.files.length} file${st.files.length === 1 ? '' : 's'}` : 'Uncommitted'),
    ...(st.files.length ? st.files.map(f => fileRow(f, false)) : [h('p', { class: 'cap' }, 'Nothing uncommitted.')]),
    st.files.length ? h('form', { class: 'commit-form', onsubmit: e => {
      e.preventDefault();
      api('POST', `sessions/${m.id}/git/commit`, { message: msg.value }).then(r => { toast('Committed ' + r.sha); renderGit(box, m); }).catch(fail);
    } }, msg, h('div', { class: 'acts' }, h('button', { class: 'primary btn small' }, 'Commit all'))) : null,
    ...(wt ? [
      h('h3', {}, `On ${st.branch} · ${st.committed.length} file${st.committed.length === 1 ? '' : 's'}`),
      ...(st.committed.length ? st.committed.map(f => fileRow(f, true)) : [h('p', { class: 'cap' }, 'No commits on this branch yet.')]),
      h('div', { class: 'acts git-acts' },
        h('button', { class: 'primary btn small', disabled: !st.ahead || st.merged, title: st.merged ? 'Already merged' : st.ahead ? `Merge ${st.branch} into ${wt.base} in ${wt.repo}` : 'Commit something first',
          onclick: () => api('POST', `sessions/${m.id}/git/merge`).then(r => { toast(`Merged into ${wt.base} (${r.sha})`); renderGit(box, m); }).catch(fail) }, `Merge into ${wt.base}`),
        h('button', { class: 'btn small', onclick: async () => {
          const keep = await ask(`The folder ${wt.path} goes away and the session is archived. Keep the branch ${wt.branch}?`,
            { title: 'Remove this worktree?', ok: 'Remove, keep branch' });
          if (keep) api('POST', `sessions/${m.id}/git/remove`, { delete_branch: false }).then(() => toast('Worktree removed')).catch(fail);
        } }, 'Remove worktree')),
    ] : [])].filter(Boolean));
}

/* ---------- drawer ---------- */
function showDrawer(tab) {
  S.drawer = true;
  if (tab) S.tab = tab;
  remember('drawer', '1');
  remember('tab', S.tab);
  $('#drawer').hidden = false;
  $('#drawer-btn').classList.add('on');
  $$('.tabs button').forEach(b => b.classList.toggle('on', b.dataset.tab === S.tab));
  renderPane();
}
function hideDrawer() {
  S.drawer = false;
  remember('drawer', '0');
  $('#drawer').hidden = true;
  $('#drawer-btn').classList.remove('on');
}
function verifyBlock(email, url) {
  return h('div', { class: 'verify-box' },
    h('p', {}, 'Google wants this account verified before Antigravity serves it. Open the link signed in as ',
      h('b', {}, email), ', finish the steps, then check again. antiquota also re-checks it by itself every 10 minutes.'),
    h('div', { class: 'card-acts' },
      h('a', { class: 'primary btn small', href: url, target: '_blank', rel: 'noopener noreferrer' }, 'Verify with Google'),
      h('button', {
        class: 'btn small', onclick: async e => {
          e.target.disabled = true;
          e.target.textContent = 'Checking…';
          try {
            S.accounts = await api('POST', `accounts/${encodeURIComponent(email)}/check`);
            renderRelay();
            toast(S.accounts.find(x => x.slot === email)?.verify ? 'Still not verified' : `${email} is ready`);
          } catch (err) { fail(err); }
          renderPane();
        },
      }, 'Check again')));
}
/* Tokens per day through antiquota (one series: magnitude over time), today in the accent.
   Hover a day for the split by account; the table below holds the same numbers per account. */
function renderUsage(u) {
  const box = $('#usage');
  if (!box) return;
  const days = u.days, max = Math.max(1, ...days.map(d => d.total));
  const total = days.reduce((s, d) => s + d.total, 0), turns = days.reduce((s, d) => s + d.turns, 0);
  const W = 336, H = 72, gap = 2, bw = (W - gap * (days.length - 1)) / days.length;
  const NS = 'http://www.w3.org/2000/svg', svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', `0 0 ${W} ${H + 1}`);
  svg.setAttribute('class', 'ubars');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', `Tokens per day, last ${days.length} days; total ${fmtK(total)}`);
  const tip = h('div', { class: 'utip', hidden: '' });
  const el = (tag, attrs) => { const n = document.createElementNS(NS, tag); for (const k in attrs) n.setAttribute(k, attrs[k]); return n; };
  svg.append(el('line', { x1: 0, x2: W, y1: H + .5, y2: H + .5, class: 'ubase' }));
  days.forEach((d, i) => {
    const x = i * (bw + gap), hgt = d.total ? Math.max(3, d.total / max * (H - 4)) : 0, today = i === days.length - 1;
    if (hgt) {  // 4px rounded top, square base on the baseline
      const r = Math.min(4, bw / 2, hgt), y = H - hgt;
      svg.append(el('path', { class: 'ubar' + (today ? ' today' : ''),
        d: `M${x},${H} V${y + r} Q${x},${y} ${x + r},${y} H${x + bw - r} Q${x + bw},${y} ${x + bw},${y + r} V${H} Z` }));
    }
    const hit = el('rect', { x, y: 0, width: bw + gap, height: H, class: 'uhit' });
    hit.addEventListener('mouseenter', () => {
      const by = Object.entries(d.by).sort((a, b) => b[1] - a[1]);
      tip.replaceChildren(h('b', {}, new Date(d.date + 'T12:00').toLocaleDateString([], { weekday: 'short', month: 'short', day: 'numeric' })),
        h('div', {}, `${fmtK(d.total)} tokens · ${d.turns} turn${d.turns === 1 ? '' : 's'}`),
        ...by.map(([slot, n]) => h('div', { class: 'ur' }, h('span', {}, short(slot)), h('span', {}, fmtK(n)))));
      tip.hidden = false;
      tip.style.left = Math.min(Math.max(0, (x + bw / 2) / W * 100), 100) + '%';
    });
    hit.addEventListener('mouseleave', () => { tip.hidden = true; });
    svg.append(hit);
  });
  const fmtDay = s => new Date(s + 'T12:00').toLocaleDateString([], { month: 'short', day: 'numeric' });
  const accts = Object.entries(u.accounts).sort((a, b) => b[1].tokens - a[1].tokens);
  box.replaceChildren(
    h('div', { class: 'uhero' }, h('b', {}, fmtK(total)), h('span', {}, ` tokens · ${turns} turn${turns === 1 ? '' : 's'}`),
      h('span', { class: 'umax' }, 'peak ' + fmtK(max === 1 ? 0 : max) + '/day')),
    h('div', { class: 'uplot' }, svg, tip),
    h('div', { class: 'uaxis' }, h('span', {}, fmtDay(days[0].date)), h('span', {}, 'today')),
    accts.length ? h('table', { class: 'utable' },
      h('thead', {}, h('tr', {}, h('th', {}, 'Account'), h('th', {}, '14 days'), h('th', {}, 'Today'), h('th', {}, 'Turns'))),
      h('tbody', {}, accts.map(([slot, t]) => h('tr', {}, h('td', { title: slot }, short(slot)), h('td', {}, fmtK(t.tokens)),
        h('td', {}, t.today ? fmtK(t.today) : '–'), h('td', {}, String(t.turns))))))
      : h('p', { class: 'cap' }, 'No turns in this period yet.'));
}
const ROTATION_CAP = {
  perishable: 'Rotation spends the allowance that resets soonest first, so no quota expires unused.',
  sticky: 'Rotation drains accounts one by one, in order.',
  'round-robin': 'Rotation takes the least recently used account.',
};
const short = s => (s || '').split('@')[0];
const shortWhy = w => !w ? 'walled' : /^rate limited/.test(w) ? 'resting' : /^signed out/.test(w) ? 'signed out' : /verify/.test(w) ? 'verify' : /errors/.test(w) ? 'resting' : 'walled';
function winLabel(w) { return w === '5h' ? '5 hour' : w; }
async function renderPane() {
  const pane = $('#pane');
  if (S.tab === 'accounts') {
    pane.replaceChildren(...[
      h('p', { class: 'cap' }, ROTATION_CAP[S.defaults.rotation || 'perishable'] + ' A session stays on its account while it has headroom; below 3% it moves on before failing mid-turn. A spent pool is walled until its reset; a short rate limit only rests the account.'),
      !S.accounts.length && h('p', { class: 'cap' }, 'No account yet. Add one: antiquota signs it in separately from your agy CLI.'),
      ...S.accounts.map(acct => h('div', { class: 'card' + (acct.verify ? ' verify' : '') },
        h('div', { class: 'card-head' }, h('b', {}, acct.slot, acct.paused ? h('em', { class: 'paused-tag' }, 'paused') : null),
          h('span', {}, (acct.busy ? 'running · ' : '') + (acct.probed ? 'probed ' + ago(acct.probed) : 'not probed'))),
        acct.verify && verifyBlock(acct.slot, acct.verify),
        acct.today && acct.today.turns ? h('div', { class: 'today', title: 'Tokens this account used today through antiquota' },
          `today  ↑ ${fmtK(acct.today.input)}  ↓ ${fmtK(acct.today.output)}` + (acct.today.thinking ? `  ✻ ${fmtK(acct.today.thinking)}` : '') +
          `  · ${acct.today.turns} turn${acct.today.turns > 1 ? 's' : ''}`) : null,
        !acct.verify && Object.entries({ gemini: 'Gemini', cg: 'Claude / GPT' }).map(([b, label]) => h('div', { class: 'pool' },
          h('div', { class: 'pool-name' }, h('span', {}, label), acct.walls[b] ? h('em', { title: (acct.why || {})[b] || '' }, `${shortWhy((acct.why || {})[b])} until ${clock(acct.walls[b])}`) : null),
          Object.entries(acct.quota[b] || {}).map(([w, [frac, reset]]) => h('div', { class: 'win', title: reset ? 'resets ' + new Date(reset * 1000).toLocaleString() : 'disabled until the other window resets' },
            h('span', {}, winLabel(w)),
            h('div', { class: 'bar' + (frac <= .02 ? ' wall' : frac < .2 ? ' low' : '') }, h('i', { style: `width:${Math.max(2, frac * 100)}%` })),
            h('span', { class: 'pct' }, Math.round(frac * 100) + '%'))))),
        h('div', { class: 'card-acts' },
          h('button', { class: 'btn small', title: acct.paused ? 'Put it back into rotation' : 'Keep it out of rotation (for example to save it for something else)',
            onclick: () => api('POST', `accounts/${encodeURIComponent(acct.slot)}/pause`, { on: !acct.paused }).then(a => { S.accounts = a; renderRelay(); }).catch(fail) },
            acct.paused ? 'Resume' : 'Pause'),
          h('button', { class: 'btn small', title: 'Read its quota and sign-in state now', onclick: () => api('POST', `accounts/${encodeURIComponent(acct.slot)}/check`).then(a => { S.accounts = a; renderRelay(); toast('Checked ' + acct.slot); }).catch(fail) }, 'Check'),
          h('button', { class: 'btn small', title: 'Sign in with the same Google account again (expired token)', onclick: () => openAcct() }, 'Sign in again'),
          h('button', {
            class: 'btn small', onclick: async () => await ask('Your agy CLI keeps its own sign-in. You can add the account again later.',
              { title: `Remove ${acct.slot}?`, ok: 'Remove', danger: true })
              && api('DELETE', 'accounts/' + encodeURIComponent(acct.slot)).then(() => toast('Removed ' + acct.slot)).catch(fail),
          }, 'Remove')))),
      h('div', { class: 'add-row' },
        h('button', { class: 'primary', onclick: () => openAcct() }, 'Add account'),
        h('button', { class: 'btn', onclick: () => api('POST', 'accounts/probe').then(() => toast('Probing accounts…')).catch(fail) }, 'Probe now')),
      h('h3', {}, 'Usage · last 14 days'),
      h('div', { class: 'usage', id: 'usage' }, h('p', { class: 'cap' }, 'Loading…')),
      h('h3', {}, 'Routing'),
      h('div', { class: 'routing', id: 'routing' }, h('p', { class: 'cap' }, 'Loading…')),
    ].filter(Boolean));
    api('GET', 'usage?days=14').then(renderUsage).catch(() => {});
    api('GET', 'routing?n=40').then(rows => {
      const box = $('#routing');
      if (!box) return;
      const title = sid => S.sessions.get(sid)?.title || sid;
      const line = e => ({
        pick: [`→ ${short(e.slot)}`, `serves ${title(e.session)}` + (e.previous ? ` (was ${short(e.previous)})` : '')],
        wall: [`▮ ${short(e.slot)}`, `${e.why}, walled until ${clock(e.until)}`],
        cooldown: [`◌ ${short(e.slot)}`, `${e.why}, rests ${Math.round(e.secs / 60) || 1} min`],
        retry: [`↻ ${short(e.slot)}`, `temporary error, retry in ${e.wait}s`],
        'signed-out': [`✕ ${short(e.slot)}`, 'sign-in rejected: sign in again'],
        paused: [`⏸ ${short(e.slot)}`, 'paused by you'],
        resumed: [`▶ ${short(e.slot)}`, 'resumed by you'],
      })[e.event] || [e.event, e.slot || ''];
      box.replaceChildren(...(rows.length ? rows.slice().reverse().map(e => {
        const [k, text] = line(e);
        return h('div', { class: 'rt ' + e.event, title: e.why || '' }, h('span', { class: 't' }, clock(e.ts)), h('span', { class: 'k' }, k), h('span', {}, text));
      }) : [h('p', { class: 'cap' }, 'No routing decisions yet.')]));
    }).catch(() => {});
  } else if (S.tab === 'files') {
    const m = cur();
    if (!m) return pane.replaceChildren(h('p', { class: 'cap' }, 'Open a session to browse its folder.'));
    renderTree(pane, m, S.treePath && S.treeFor === m.id ? S.treePath : '');
  } else if (S.tab === 'changes') {
    const files = new Map();
    for (const e of S.events) {
      if (e.t !== 'tool' || !e.diff) continue;
      const path = e.params.TargetFile || e.params.File || '?';
      const f = files.get(path) || { path, add: 0, del: 0, diffs: [], last: 0 };
      const [ad, de] = diffStats(e.diff);
      f.add += ad; f.del += de; f.diffs.push(e.diff); f.last = e.ts || f.last;
      files.set(path, f);
    }
    const list = [...files.values()].sort((x, y) => y.last - x.last);
    const gitBox = h('div', { class: 'gitbox' });
    if (cur()) renderGit(gitBox, cur());
    pane.replaceChildren(
      gitBox,
      h('h3', {}, 'Edits by agy in this session'),
      h('p', { class: 'cap' }, cur() ? `Files this session changed, newest first. Click one for its diffs.` : 'Open a session to see its changes.'),
      ...(list.length ? list.map(f => {
        const row = h('details', { class: 'change' },
          h('summary', {}, h('span', { class: 'cpath', title: f.path }, rel(f.path)),
            h('span', { class: 'meta' }, f.add ? h('span', { class: 'stat add' }, '+' + f.add) : null,
              f.del ? h('span', { class: 'stat del' }, '−' + f.del) : null, `${f.diffs.length}×`)));
        row.addEventListener('toggle', () => {
          if (row.open && row.children.length === 1) row.append(...f.diffs.map(d => diffNode(d)));
        }, { once: false });
        return row;
      }) : [h('p', { class: 'cap' }, 'No file edits yet.')]));
  } else {
    pane.replaceChildren(h('p', { class: 'cap' }, 'Loading what agy reports…'));
    try {
      S.stack = S.stack || await api('GET', 'stack');
    } catch (e) { pane.replaceChildren(h('p', { class: 'cap' }, e.message)); return; }
    const st = S.stack;
    pane.replaceChildren(
      h('p', { class: 'cap' }, 'antiquota runs on its own copy of your agy setup. Every account loads it; your agy CLI and the IDE are never changed.'),
      h('h3', {}, `Skills · ${(st.skills || []).length}`), h('div', { class: 'chips' }, (st.skills || []).map(x => h('span', { class: 'chip' }, x))),
      h('h3', {}, 'Hooks'), h('div', { class: 'chips' }, (st.hooks || []).map(x => h('span', { class: 'chip' }, x))),
      h('h3', {}, 'MCP servers'), h('div', { class: 'chips' }, (st.mcp || []).map(x => h('span', { class: 'chip' }, x))),
      h('div', { class: 'add-row' },
        h('button', { class: 'primary', onclick: () => openSettings() }, 'Open settings'),
        h('button', { class: 'btn', onclick: async () => { S.stack = null; S.stack = await api('GET', 'stack?refresh=1').catch(fail); renderPane(); } }, 'Refresh')));
  }
}

/* ---------- dialogs ---------- */
async function browse(path) {
  const box = $('#browser');
  let r;
  try { r = await api('GET', 'fs?path=' + encodeURIComponent(path || '')); } catch { return; }
  const recent = [...new Set([...S.sessions.values()].sort((a, b) => b.updated - a.updated).map(m => m.cwd))].slice(0, 4);
  const go = p => { $('#new-cwd').value = p; browse(p); };
  box.replaceChildren(...[
    ...(path ? [] : recent.map(p => h('button', { type: 'button', class: 'recent', onclick: () => go(p) }, '★ ' + p.replace(/^\/Users\/[^/]+/, '~')))),
    r.parent && h('button', { type: 'button', class: 'up', onclick: () => go(r.parent) }, '↰ ..'),
    ...r.dirs.map(d => h('button', { type: 'button', onclick: () => go(r.path + '/' + d) }, d + '/')),
  ].filter(Boolean));
  box.hidden = !box.children.length;
}
let pendingText = '';
function openNew(text) {
  pendingText = typeof text === 'string' ? text : '';
  const m = cur();
  $('#new-cwd').value = m ? (m.worktree ? m.worktree.repo : m.cwd) : S.defaults.cwd;
  $('#new-worktree').checked = false;
  setPicker($('#new-model'), $('#new-effort'), m ? m.model : S.defaults.model);
  $('#new-mode').value = m ? m.mode : (S.defaults.mode || 'default');
  browse('');
  $('#new-dlg').showModal();
  $('#new-cwd').select();
}
async function createFromDialog() {
  try {
    const meta = await api('POST', 'sessions', {
      cwd: $('#new-cwd').value.trim(), model: $('#new-effort').value, mode: $('#new-mode').value,
      worktree: $('#new-worktree').checked,
    });
    S.sessions.set(meta.id, meta);
    await openSession(meta.id);
    if (pendingText) { $('#input').value = pendingText; pendingText = ''; await submit(); }
  } catch (e) { fail(e); }
}
function openGoal() {
  const m = cur();
  if (!m) return openNew();
  const g = m.goal || {};
  $('#goal-objective').value = g.objective || $('#input').value.trim();
  $('#goal-check').value = g.check || '';
  $('#goal-verifier').checked = g.verifier ?? true;
  setModelSelect($('#goal-vmodel'), g.verifier_model || S.defaults.verifier_model);
  $('#goal-rounds').value = g.max_rounds || 20;
  $('#goal-dlg').showModal();
}
async function startGoal() {
  const m = cur();
  try {
    await api('POST', `sessions/${m.id}/goal`, {
      objective: $('#goal-objective').value, check: $('#goal-check').value, verifier: $('#goal-verifier').checked,
      verifier_model: $('#goal-vmodel').value, max_rounds: +$('#goal-rounds').value || 20,
    });
    if ($('#input').value.trim() === $('#goal-objective').value.trim()) { $('#input').value = ''; autosize(); }
    toast('Goal started');
  } catch (e) { fail(e); }
}

/* ---------- add account: open Google sign-in, paste the code ---------- */
const L = { id: null, timer: null, poll: null, until: 0 };
function acctMsg(text, cls = '') { const m = $('#acct-msg'); m.textContent = text; m.className = 'acct-msg ' + cls; }
function openAcct() {
  stopLogin();
  $('#acct-code').value = '';
  $('#acct-code').disabled = true;
  $('#acct-finish').disabled = true;
  $('#acct-open').textContent = 'Open Google sign-in';
  $('#acct-timer').textContent = '';
  acctMsg('');
  $('#acct-dlg').showModal();
  $('#acct-open').focus();
}
function stopLogin(cancel = true) {
  clearInterval(L.timer);
  clearInterval(L.poll);
  if (cancel && L.id) api('DELETE', 'accounts/login/' + L.id).catch(() => {});
  L.id = null;
}
async function startLogin() {
  stopLogin();
  // Open the tab inside the click, or the browser blocks it; point it at Google once agy gives the URL.
  let w = null;
  if (!NATIVE) try { w = window.open('', '_blank'); if (w) w.opener = null; } catch { w = null; }
  acctMsg('Starting agy sign-in…');
  let r;
  try { r = await api('POST', 'accounts/login', {}); } catch (e) {
    try { w?.close(); } catch {}
    return acctMsg(e.message, 'bad');
  }
  L.id = r.id;
  L.until = Date.now() + r.seconds * 1000;
  let opened = false;
  try { if (w && !w.closed) { w.location.href = r.url; opened = true; } } catch {}
  if (NATIVE) { openExternal(r.url); opened = true; }
  const link = text => h('a', { href: r.url, target: '_blank', rel: 'noopener noreferrer' }, text);
  acctMsg('');
  $('#acct-msg').replaceChildren(...(opened
    ? ['Sign in in the new tab, then paste the code here. ', link('Reopen the page')]
    : ['Open ', link('the Google sign-in page'), ', sign in, then paste the code here.']));
  $('#acct-open').textContent = 'New sign-in link';
  $('#acct-code').disabled = false;
  $('#acct-code').focus();
  $('#acct-finish').disabled = false;
  const tick = () => {
    const left = Math.max(0, Math.round((L.until - Date.now()) / 1000));
    $('#acct-timer').textContent = left ? left + 's left' : 'link expired';
    $('#acct-timer').classList.toggle('low', left <= 15);
  };
  tick();
  L.timer = setInterval(tick, 500);
  // agy may also notice the sign-in by itself; then no code is needed.
  L.poll = setInterval(async () => {
    if (!L.id) return;
    try { loginState(await api('GET', 'accounts/login/' + L.id)); } catch {}
  }, 2000);
}
function loginState(r) {
  if (r.state === 'done') {
    stopLogin(false);
    acctMsg(`Signed in: ${r.name}. It joins the rotation now.`, 'ok');
    refreshAccounts();
    setTimeout(() => $('#acct-dlg').open && $('#acct-dlg').close('done'), 1200);
  } else if (r.state === 'verify') {
    stopLogin(false);
    $('#acct-code').disabled = true;
    $('#acct-finish').disabled = true;
    $('#acct-timer').textContent = '';
    refreshAccounts();
    $('#acct-msg').className = 'acct-msg';
    $('#acct-msg').replaceChildren(verifyBlock(r.name, r.verify_url));
  } else if (r.state === 'failed') {
    stopLogin(false);
    $('#acct-code').disabled = true;
    $('#acct-finish').disabled = true;
    $('#acct-timer').textContent = '';
    acctMsg((r.error || 'Sign-in failed') + '. Get a new sign-in link and try again.', 'bad');
  }
}
const CODE_RE = /^4\/[0-9A-Za-z_-]{20,}$/;
async function pasteFromClipboard(quiet) {
  if (!L.id || $('#acct-code').disabled || $('#acct-code').value.trim()) return;
  try {
    const text = (await readClipboard()).trim();
    if (CODE_RE.test(text)) {
      $('#acct-code').value = text;
      acctMsg('Code taken from the clipboard…');
      finishLogin();
    } else if (!quiet) acctMsg('The clipboard holds no sign-in code yet. Click "Copy to Clipboard" on Google\'s page.', 'bad');
  } catch {
    if (!quiet) acctMsg('The browser did not allow reading the clipboard; paste the code with ⌘V.', 'bad');
  }
}
async function finishLogin() {
  const code = $('#acct-code').value.trim();
  if (!L.id || !code) return;
  $('#acct-finish').disabled = true;
  acctMsg('Checking the code…');
  try { loginState(await api('POST', 'accounts/login/' + L.id, { code })); }
  catch (e) { acctMsg(e.message, 'bad'); $('#acct-finish').disabled = false; }
}

/* ---------- settings: antiquota's own copy of your agy setup ---------- */
const SET = { data: null, section: 'general', open: null };
const SECTIONS = [['general', 'General'], ['rules', 'Rules'], ['skills', 'Skills'], ['hooks', 'Hooks'],
  ['mcp', 'MCP servers'], ['plugins', 'Plugins'], ['permissions', 'Permissions']];
const LIST = { rules: 'rules', skills: 'skills', hooks: 'hooks', mcp: 'mcps', plugins: 'plugins' };
const SRC = { cli: 'agy CLI', ide: 'IDE' };
const TEMPLATE = {
  rule: n => `---\ntrigger: always_on\n---\n\n# ${n.replace(/\.md$/, '')}\n\n`,
  skill: n => `---\nname: ${n}\ndescription: What it does and when agy should use it.\n---\n\n# ${n}\n\n`,
  hook: () => ({ PostToolUse: [{ matcher: '*', hooks: [{ type: 'command', command: '', timeout: 30 }] }], enabled: true }),
  mcp: () => ({ command: '', args: [] }),
};

async function openSettings(section) {
  if (typeof section === 'string') SET.section = section;
  SET.open = null;
  $('#settings').hidden = false;
  await loadSettings();
}
function closeSettings() { $('#settings').hidden = true; SET.open = null; }
async function loadSettings() {
  try { SET.data = await api('GET', 'settings'); } catch (e) { return fail(e); }
  S.defaults = { ...S.defaults, ...SET.data.defaults };
  renderSettings();
}
function renderSettings() {
  const d = SET.data;
  $('#set-sections').replaceChildren(...SECTIONS.map(([id, label]) => h('button', {
    class: 'set-tab' + (id === SET.section ? ' on' : ''),
    onclick: () => { SET.section = id; SET.open = null; renderSettings(); },
  }, h('span', {}, label), LIST[id] && h('span', { class: 'n' }, d[LIST[id]].filter(r => r.here).length))));
  $('#set-path').textContent = 'stack: ' + d.path.replace(/^\/Users\/[^/]+/, '~');
  $('#set-body').replaceChildren(h('div', { class: 'set-inner' }, ...RENDER[SET.section](d)));
}
const reload = () => { S.stack = null; return loadSettings(); };
async function doCopy(source, kind, name) {
  try {
    await api('POST', 'settings/copy', { source, kind, name });
    toast(`Copied ${name || (kind === 'everything' ? 'everything' : 'all ' + kind + 's')} from ${SRC[source]}`);
    await reload();
  } catch (e) { fail(e); }
}
function head(title, cap, ...acts) {
  return h('header', { class: 'set-sec-head' },
    h('div', {}, h('h2', {}, title), h('p', {}, cap)), h('div', { class: 'set-acts' }, acts));
}
function copyAll(kind, src, rows) {
  const n = rows.filter(r => r[src] === 'differs').length;
  return n > 0 && h('button', { class: 'btn small', onclick: () => doCopy(src, kind) }, `Copy ${n} from ${SRC[src]}`);
}
function switchBtn(on, set) {
  return h('button', {
    class: 'switch' + (on ? ' on' : ''), role: 'switch', 'aria-checked': String(on), title: on ? 'On' : 'Off',
    onclick: e => { e.currentTarget.classList.toggle('on'); set(!on); },
  }, h('i'));
}
function editorBox(ta, label, save) {
  return h('div', { class: 'set-editor' }, ta,
    h('div', { class: 'set-ed-acts' }, h('span', { class: 'set-ed-path' }, label),
      h('button', { class: 'ghost btn small', onclick: () => { SET.open = null; renderSettings(); } }, 'Cancel'),
      h('button', { class: 'primary btn small', onclick: save }, 'Save')));
}
function fileEditor(path, rows = 14) {
  const ta = h('textarea', { class: 'set-edit', spellcheck: 'false', rows, 'aria-label': path });
  api('GET', 'settings/file?path=' + encodeURIComponent(path)).then(r => { ta.value = r.text; }).catch(fail);
  return editorBox(ta, path, async () => {
    try { await api('PUT', 'settings/file', { path, text: ta.value }); toast('Saved ' + path); SET.open = null; await reload(); } catch (e) { fail(e); }
  });
}
function jsonEditor(kind, name) {
  const ta = h('textarea', { class: 'set-edit', spellcheck: 'false', rows: 14, 'aria-label': name });
  api('GET', `settings/${kind}/${encodeURIComponent(name)}`).then(v => { ta.value = JSON.stringify(v, null, 2); }).catch(fail);
  return editorBox(ta, `${kind === 'hook' ? 'hooks.json' : 'mcp_config.json'} → ${name}`, async () => {
    let value;
    try { value = JSON.parse(ta.value); } catch (e) { return toast('Invalid JSON: ' + e.message); }
    try { await api('PUT', `settings/${kind}/${encodeURIComponent(name)}`, { value }); toast('Saved ' + name); SET.open = null; await reload(); } catch (e) { fail(e); }
  });
}
const EDIT_PATH = { skill: n => `config/skills/${n}/SKILL.md`, rule: n => `config/rules/${n}` };
function itemRow(kind, r, sources) {
  const key = kind + ':' + r.name, open = SET.open === key;
  const enc = encodeURIComponent(r.name);
  const tags = sources.map(src => r[src] && h('span', { class: 'tag' + (r[src] === 'same' ? '' : ' warn') },
    r[src] === 'same' ? `= ${SRC[src]}` : r.here ? `≠ ${SRC[src]}` : `only in ${SRC[src]}`));
  const acts = [
    'enabled' in r && switchBtn(r.enabled, on => api('POST', `settings/${kind}/${enc}/enabled`, { on }).then(reload).catch(fail)),
    r.here && kind !== 'plugin' && h('button', { class: 'btn small', onclick: () => { SET.open = open ? null : key; renderSettings(); } }, open ? 'Close' : 'Edit'),
    ...sources.map(src => r[src] === 'differs' && h('button', { class: 'btn small', onclick: () => doCopy(src, kind, r.name) },
      (r.here ? 'Replace from ' : 'Copy from ') + SRC[src])),
    r.here && h('button', {
      class: 'btn small danger',
      onclick: async () => await ask('Your agy CLI and the IDE keep theirs.', { title: `Delete ${kind} "${r.name}" from antiquota?`, ok: 'Delete', danger: true })
        && api('DELETE', `settings/${kind}/${enc}`).then(reload).catch(fail),
    }, 'Delete'),
  ];
  return h('div', { class: 'set-item' + (open ? ' open' : '') },
    h('div', { class: 'set-row' + (r.here ? '' : ' absent') },
      h('div', { class: 'set-main' }, h('div', { class: 'set-name' }, h('b', {}, r.name), tags), r.about && h('div', { class: 'set-about' }, r.about)),
      h('div', { class: 'set-acts' }, acts)),
    open && (EDIT_PATH[kind] ? fileEditor(EDIT_PATH[kind](r.name)) : jsonEditor(kind, r.name)));
}
function newItem(label, placeholder, create) {
  const input = h('input', { placeholder, spellcheck: 'false', 'aria-label': label });
  return h('form', {
    class: 'add-row set-new',
    onsubmit: async e => {
      e.preventDefault();
      const n = input.value.trim();
      if (!/^[A-Za-z0-9._:@+-]+$/.test(n) || n.startsWith('.')) return toast('Name: letters, digits and . _ : @ + -');
      try { await create(n); } catch (err) { fail(err); }
    },
  }, input, h('button', { class: 'btn' }, label));
}
function list(kind, rows, sources) {
  return rows.length ? rows.map(r => itemRow(kind, r, sources)) : [h('p', { class: 'cap' }, 'Nothing here yet.')];
}
const openNewItem = async (key) => { SET.open = key; await reload(); };
function autostartRow() {
  const row = h('div', { class: 'set-toggle' });
  NATIVE.autostart.isEnabled().then(on => row.replaceChildren(
    switchBtn(on, v => (v ? NATIVE.autostart.enable() : NATIVE.autostart.disable()).catch(fail)),
    h('div', {}, h('b', {}, 'Open at login'), h('p', { class: 'cap' }, 'antiquota starts in the menu bar / tray when you log in, ready for sessions and schedules.'))))
    .catch(() => row.remove());
  return row;
}
function defaultsForm(df) {
  const fam = h('select', { id: 'df-family' }), eff = h('select', { id: 'df-effort' });
  const mode = h('select', { id: 'df-mode' }, [['default', 'default'], ['accept-edits', 'accept edits'], ['plan', 'plan'], ['bypass', 'bypass']]
    .map(([v, t]) => h('option', { value: v }, t)));
  const ver = h('select', { id: 'df-vmodel' });
  const rot = h('select', { id: 'df-rotation' }, [['perishable', 'soonest reset first'], ['sticky', 'one by one'], ['round-robin', 'round robin']]
    .map(([v, t]) => h('option', { value: v }, t)));
  rot.value = df.rotation || 'perishable';
  setPicker(fam, eff, df.model);
  fam.addEventListener('change', () => pickFamily(fam, eff, eff.value));
  mode.value = df.mode;
  setModelSelect(ver, df.verifier_model);
  return h('div', { class: 'set-block' },
    h('h3', {}, 'New sessions'),
    h('div', { class: 'row3 row4' },
      h('label', { class: 'field' }, h('span', {}, 'Model'), fam),
      h('label', { class: 'field' }, h('span', {}, 'Effort'), eff),
      h('label', { class: 'field' }, h('span', {}, 'Mode'), mode),
      h('label', { class: 'field' }, h('span', {}, 'Goal verifier'), ver)),
    h('h3', {}, 'Accounts'),
    h('div', { class: 'row3' }, h('label', { class: 'field' }, h('span', {}, 'Rotation'), rot)),
    h('p', { class: 'cap' }, 'Sessions keep their account while it has headroom; rotation decides which account a session moves to.'),
    h('h3', {}, 'This computer'),
    h('div', { class: 'set-toggle' },
      switchBtn((df.keep_awake || 'on') === 'on', on => api('PUT', 'settings/defaults', { keep_awake: on ? 'on' : 'off' })
        .then(d => { S.defaults = { ...S.defaults, ...d }; }).catch(fail)),
      h('div', {}, h('b', {}, 'Keep awake while agents work'), h('p', { class: 'cap' }, 'The computer does not sleep while a session or background job runs. The display may still turn off.'))),
    NATIVE && autostartRow(),
    h('div', { class: 'add-row' }, h('button', {
      class: 'primary', onclick: async () => {
        try {
          S.defaults = { ...S.defaults, ...await api('PUT', 'settings/defaults', { model: eff.value, mode: mode.value, verifier_model: ver.value, rotation: rot.value }) };
          toast('Defaults saved');
        } catch (e) { fail(e); }
      },
    }, 'Save defaults')));
}
function permList(kind, d) {
  const rules = d.permissions[kind];
  const input = h('input', { placeholder: kind === 'allow' ? 'command(npm test)' : 'command(rm)', spellcheck: 'false', 'aria-label': `New ${kind} rule` });
  const save = next => api('PUT', 'permissions', { [kind]: next }).then(reload).catch(fail);
  return [h('h3', {}, kind === 'allow' ? 'Allow' : 'Deny'),
    ...(rules.length ? rules.map(r => h('div', { class: 'rule-row' }, h('span', {}, r),
      h('button', { title: 'Remove', 'aria-label': 'Remove ' + r, onclick: () => save(rules.filter(x => x !== r)) }, '✕')))
      : [h('p', { class: 'cap' }, 'No rules.')]),
    h('form', { class: 'add-row', onsubmit: e => { e.preventDefault(); const v = input.value.trim(); if (v) save([...rules, v]); } },
      input, h('button', { class: 'btn' }, 'Add'))];
}
const RENDER = {
  general: d => [
    head('General', 'Defaults for new sessions, and the copy of your agy setup that antiquota runs on.'),
    defaultsForm(d.defaults),
    h('div', { class: 'set-block' },
      h('h3', {}, 'antiquota stack'),
      h('p', { class: 'cap' }, `Every antiquota account runs with ${d.path.replace(/^\/Users\/[^/]+/, '~')}: your rules, skills, hooks, MCP servers, plugins and permissions, copied. Edits here never touch ~/.gemini (your agy CLI) or the Antigravity IDE, and theirs only arrive when you copy them.`),
      h('div', { class: 'add-row' }, h('button', {
        class: 'btn', onclick: async () => await ask('Edits made here are lost; conversations stay.',
          { title: "Replace antiquota's config with a fresh copy of your agy CLI setup?", ok: 'Replace', danger: true }) && doCopy('cli', 'everything'),
      }, 'Copy everything again from agy CLI'))),
  ],
  rules: d => [
    head('Rules', 'GEMINI.md is always loaded. Files in rules/ load by their trigger (always_on, model_decision, glob).',
      d.gemini_md.cli === 'differs' && h('button', { class: 'btn small', onclick: () => doCopy('cli', 'gemini_md') }, 'GEMINI.md from agy CLI'),
      copyAll('rule', 'cli', d.rules)),
    h('h3', {}, 'GEMINI.md'), fileEditor('GEMINI.md', 10),
    h('h3', {}, 'rules/'), ...list('rule', d.rules, ['cli']),
    newItem('New rule', 'testing.md', async n => {
      n = n.endsWith('.md') ? n : n + '.md';
      await api('PUT', 'settings/file', { path: `config/rules/${n}`, text: TEMPLATE.rule(n) });
      await openNewItem('rule:' + n);
    }),
  ],
  skills: d => [
    head('Skills', 'A skill is a folder with SKILL.md. agy sees every name and description, and loads the body when it needs it.',
      copyAll('skill', 'cli', d.skills), copyAll('skill', 'ide', d.skills)),
    ...list('skill', d.skills, ['cli', 'ide']),
    newItem('New skill', 'my-skill', async n => {
      await api('PUT', 'settings/file', { path: `config/skills/${n}/SKILL.md`, text: TEMPLATE.skill(n) });
      await openNewItem('skill:' + n);
    }),
  ],
  hooks: d => [
    head('Hooks', 'Commands agy runs on tool and turn events (hooks.json). Switch one off to keep it without running it.',
      copyAll('hook', 'cli', d.hooks)),
    ...list('hook', d.hooks, ['cli']),
    newItem('New hook', 'my-hook', async n => {
      await api('PUT', `settings/hook/${encodeURIComponent(n)}`, { value: TEMPLATE.hook() });
      await openNewItem('hook:' + n);
    }),
  ],
  mcp: d => [
    head('MCP servers', 'Tool servers agy starts for every turn (mcp_config.json): command and args, or a url.',
      copyAll('mcp', 'cli', d.mcps), copyAll('mcp', 'ide', d.mcps)),
    ...list('mcp', d.mcps, ['cli', 'ide']),
    newItem('Add server', 'server-name', async n => {
      await api('PUT', `settings/mcp/${encodeURIComponent(n)}`, { value: TEMPLATE.mcp() });
      await openNewItem('mcp:' + n);
    }),
  ],
  plugins: d => [
    head('Plugins', 'Bundles of skills and commands. Install new ones with your agy CLI, then copy them here.',
      copyAll('plugin', 'cli', d.plugins)),
    ...list('plugin', d.plugins, ['cli']),
  ],
  permissions: d => [
    head('Permissions', 'Headless agy cannot ask: anything not allowed is denied and ends the turn. These rules apply to every antiquota account, not to your agy CLI.',
      d.permissions.cli === 'differs' && h('button', { class: 'btn small', onclick: () => doCopy('cli', 'permissions') }, 'Copy from agy CLI')),
    ...permList('allow', d), ...permList('deny', d),
  ],
};

/* ---------- detail level: compact rows, or previews + thinking like Claude Code ---------- */
function setDetail(mode) {
  S.detail = mode;
  remember('detail', mode);
  document.body.classList.toggle('compact', mode === 'compact');
  $('#detail-btn').classList.toggle('on', mode === 'detailed');
  $('#detail-btn').title = mode === 'detailed' ? 'Showing previews and thinking (click for compact)' : 'Compact (click for previews and thinking)';
  $$('details.thinking').forEach(d => { d.open = mode === 'detailed' && d.textContent.length < 1300; });
}
function expandAll(open) {
  $$('.tool', $('#transcript')).forEach(t => t.toggle && t.toggle(open));
  $$('details.thinking', $('#transcript')).forEach(d => { d.open = open; });
}

/* ---------- token counter: context, session totals, the serving account's quota ---------- */
const CONTEXT_WINDOW = id => /^claude/.test(id) ? 200000 : /^gpt-oss/.test(id) ? 131072 : 1048576;
function renderMeter() {
  const m = cur(), el = $('#meter');
  if (!m) { el.replaceChildren(); return; }
  const win = CONTEXT_WINDOW(m.model), ctx = m.context || 0, pct = Math.min(100, Math.round(ctx / win * 100));
  const acct = S.accounts.find(x => x.slot === m.last_slot) || S.accounts[0];
  const pool = POOL(m.model);
  const q = acct && acct.quota[pool];
  const left = q ? Math.round(minFrac(q) * 100) : null;
  const free = S.accounts.filter(x => !x.walls[pool] && !x.verify && !x.paused).length;
  const level = left == null ? '' : left < 10 ? 'bad' : left < 25 ? 'warn' : 'ok';
  el.replaceChildren(
    h('span', { class: 'seg ctxm' + (pct >= 80 ? ' hot' : pct >= 50 ? ' warm' : ''), title: `Context sent with the last request: ${ctx.toLocaleString()} of ${win.toLocaleString()} tokens` },
      h('span', { class: 'ctx-bar', 'aria-hidden': 'true' }, h('i', { style: `width:${Math.max(pct, ctx ? 2 : 0)}%` })),
      ctx ? [h('b', {}, fmtK(ctx)), h('span', { class: 'lbl' }, ` of ${fmtK(win)} context`)] : 'no context yet'),
    h('span', { class: 'seg tok', title: 'Tokens this session: sent ↑, received ↓, thinking ✻' },
      '↑', h('b', {}, fmtK(m.usage.input)), ' ↓', h('b', {}, fmtK(m.usage.output)),
      m.usage.thinking ? [' ✻', h('b', {}, fmtK(m.usage.thinking))] : null),
    acct && h('span', { class: 'seg acct-q', title: `${acct.slot} serves this session · ${POOL_LABEL[pool]} left (lowest window)` },
      h('i', { class: 'qdot ' + level, 'aria-hidden': 'true' }), h('span', { class: 'nm' }, who(acct.slot)),
      h('b', {}, (pool === 'cg' ? 'Claude/GPT ' : 'Gemini ') + (left == null ? '–' : left + '%'))),
    h('span', { class: 'grow' }),
    h('span', { class: 'seg free', title: `Accounts that can take a ${POOL_LABEL[pool]} turn now` }, h('b', {}, `${free}/${S.accounts.length}`), ' accounts free'));
}

/* ---------- wiring ---------- */
async function load() {
  const st = await api('GET', 'state');
  S.models = st.models;
  S.defaults = st.defaults;
  S.accounts = st.accounts;
  S.sessions = new Map(st.sessions.map(m => [m.id, m]));
  fillModels();
  renderRelay();
  const want = S.cur || location.hash.slice(1);
  if (want && S.sessions.has(want)) await openSession(want);
  else if (st.sessions.length) await openSession(st.sessions[0].id);
  else { renderSessions(); renderBlank(); }
  if (!S.models.length) setTimeout(() => api('GET', 'state').then(s => { S.models = s.models; if (S.models.length) { fillModels(); renderHeader(); } }), 4000);
}

function wire() {
  const input = $('#input');
  $('#composer').addEventListener('submit', e => { e.preventDefault(); submit(); });
  input.addEventListener('input', () => { HIST.pos = -1; autosize(); openSlashMenu(); syncSend(); });
  input.addEventListener('keydown', e => {
    const menu = !$('#slash').hidden && menuItems.length;
    if (menu && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault();
      const n = menuItems.length;
      slashIdx = (slashIdx + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
      showMenu(menuItems);
      return;
    }
    if (menu && (e.key === 'Tab' || e.key === 'Enter')) {
      const isSlashWithArgs = input.value.startsWith('/') && /\s/.test(input.value);
      if (!isSlashWithArgs) {
        e.preventDefault();
        menuItems[slashIdx]?.[2]();
        closeSlash();
        syncSend();
        return;
      }
    }
    if (!menu && (e.key === 'ArrowUp' || e.key === 'ArrowDown') && !e.shiftKey
        && (input.value === '' || HIST.pos !== -1 || (e.key === 'ArrowUp' && input.selectionStart === 0))) {
      if (historyStep(e.key === 'ArrowUp' ? 1 : -1)) { e.preventDefault(); return; }
    }
    if (e.key === 'Escape') {
      if (menu) return closeSlash();
      const m = cur();
      if (m && m.status !== 'idle') { e.preventDefault(); stop(); }
      return;
    }
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); submit(); }
  });
  input.addEventListener('blur', () => setTimeout(closeSlash, 100));
  $('#model').addEventListener('change', () => patch({ model: pickFamily($('#model'), $('#effort'), cur()?.model || S.defaults.model) }));
  $('#effort').addEventListener('change', e => patch({ model: e.target.value }));
  $('#new-model').addEventListener('change', () => pickFamily($('#new-model'), $('#new-effort'), $('#new-effort').value));
  $('#mode').addEventListener('change', async e => {
    const want = e.target.value;
    e.target.value = cur()?.mode || 'default';
    if (want === 'bypass' && !await ask('The agent runs any command and edits any file without asking you.',
      { title: 'Switch to bypass?', ok: 'Bypass', danger: true })) return;
    e.target.value = want;
    patch({ mode: want });
  });
  $('#goal-btn').addEventListener('click', openGoal);
  $('#new-btn').addEventListener('click', () => openNew());
  $('#search').addEventListener('input', renderSessions);
  $('#relay-head').addEventListener('click', () => showDrawer('accounts'));
  $('#relay-add').addEventListener('click', () => openAcct());
  $('#drawer-btn').addEventListener('click', () => (S.drawer ? hideDrawer() : showDrawer()));
  $$('.tabs button').forEach(b => b.addEventListener('click', () => showDrawer(b.dataset.tab)));
  $('#side-open').addEventListener('click', () => $('#side').classList.add('open'));
  $('#side-close').addEventListener('click', () => $('#side').classList.remove('open'));
  $('#new-dlg').addEventListener('close', () => { if ($('#new-dlg').returnValue === 'ok') createFromDialog(); });
  $('#goal-dlg').addEventListener('close', () => { if ($('#goal-dlg').returnValue === 'ok') startGoal(); });
  $('#set-open').addEventListener('click', () => openSettings());
  $('#attach-btn').addEventListener('click', () => $('#attach-input').click());
  if (NATIVE) {  // the app's web view does not open new windows: links go to the default browser
    document.documentElement.classList.add('native');
    if (/Mac/.test(navigator.platform)) document.documentElement.classList.add('mac');
    for (const el of [$('.top'), $('.brand')]) el?.setAttribute('data-tauri-drag-region', '');
    document.addEventListener('click', e => {
      const a = e.target.closest('a[href]');
      if (!a || a.dataset.file || a.hasAttribute('download')) return;
      const url = new URL(a.href, location.href);
      if (!/^https?:$/.test(url.protocol) || (url.origin === location.origin && a.target !== '_blank')) return;
      e.preventDefault();
      openExternal(url.href);
    }, true);
  }
  $('#transcript').addEventListener('click', e => {
    const a = e.target.closest('a[data-file]');
    if (!a) return;
    e.preventDefault();
    const [p, frag] = a.dataset.file.split('#');
    let path = p;
    try { path = decodeURIComponent(p); } catch {}
    const r = (frag || '').match(/L(\d+)(?:-L?(\d+))?/) || [];
    openFile(path, [+r[1] || 0, +r[2] || 0]);
  });
  $('#viewer-close').addEventListener('click', () => $('#viewer').close());
  $('#viewer').addEventListener('click', e => { if (e.target === $('#viewer')) $('#viewer').close(); });
  $('#attach-input').addEventListener('change', e => { attachFiles([...e.target.files]); e.target.value = ''; });
  input.addEventListener('paste', e => {
    const stamp = new Date().toTimeString().slice(0, 8).replace(/:/g, '');
    const files = [...(e.clipboardData?.files || [])].map(f =>  // browsers name every pasted image image.png
      f.name === 'image.png' ? new File([f], `screenshot-${stamp}.png`, { type: f.type }) : f);
    if (files.length) { e.preventDefault(); attachFiles(files); }
  });
  let dragDepth = 0;
  const hasFiles = e => [...(e.dataTransfer?.types || [])].includes('Files');
  window.addEventListener('dragenter', e => { if (!hasFiles(e)) return; e.preventDefault(); dragDepth++; $('#dropzone').hidden = false; });
  window.addEventListener('dragover', e => { if (hasFiles(e)) e.preventDefault(); });
  window.addEventListener('dragleave', e => { if (!hasFiles(e)) return; dragDepth = Math.max(0, dragDepth - 1); if (!dragDepth) $('#dropzone').hidden = true; });
  window.addEventListener('drop', e => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    dragDepth = 0;
    $('#dropzone').hidden = true;
    attachFiles([...e.dataTransfer.files]);
  });
  $('#detail-btn').addEventListener('click', () => setDetail(S.detail === 'detailed' ? 'compact' : 'detailed'));
  $('#expand-btn').addEventListener('click', () => {
    const open = !$('#expand-btn').classList.contains('on');
    $('#expand-btn').classList.toggle('on', open);
    expandAll(open);
  });
  $('#set-close').addEventListener('click', closeSettings);
  $('#acct-open').addEventListener('click', startLogin);
  $('#acct-finish').addEventListener('click', finishLogin);
  $('#acct-code').addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); finishLogin(); } });
  $('#acct-code').addEventListener('paste', () => setTimeout(finishLogin, 50));  // seconds count: send on paste
  $('#acct-dlg').addEventListener('close', () => stopLogin());
  window.addEventListener('focus', () => pasteFromClipboard(true));  // back from Google's code page
  $('#acct-clip').addEventListener('click', () => pasteFromClipboard(false));
  let browseTimer;
  $('#new-cwd').addEventListener('input', e => { clearTimeout(browseTimer); browseTimer = setTimeout(() => browse(e.target.value), 200); });

  const title = $('#title');
  title.addEventListener('click', () => {
    if (!cur()) return;
    title.contentEditable = 'true';
    title.focus();
    document.getSelection().selectAllChildren(title);
  });
  const finish = save => {
    if (title.contentEditable !== 'true') return;
    title.contentEditable = 'false';
    const v = title.textContent.trim();
    if (save && v && v !== cur()?.title) patch({ title: v });
    else renderHeader();
  };
  title.addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); finish(true); }
    if (e.key === 'Escape') finish(false);
  });
  title.addEventListener('blur', () => finish(true));

  document.addEventListener('keydown', e => { if (e.key === 'Escape' && $('#ctx')) { e.preventDefault(); closeMenu(); } }, true);
  document.addEventListener('keydown', e => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === 'n') { e.preventDefault(); openNew(); }
    if (mod && e.key === '.') { e.preventDefault(); stop(); }
    if (mod && e.key === ',') { e.preventDefault(); openSettings(); }
    if (mod && e.key === '/') { e.preventDefault(); showShortcuts(); }
    if (mod && e.key.toLowerCase() === 'k') { e.preventDefault(); $('#search').focus(); $('#search').select(); }
    if (mod && e.key.toLowerCase() === 'j') { e.preventDefault(); $('#input').focus(); }
    if (mod && e.key.toLowerCase() === 'b' && !e.shiftKey) { e.preventDefault(); $('#drawer-btn').click(); }
    if (mod && e.shiftKey && (e.key === '[' || e.key === ']' || e.key === '{' || e.key === '}')) {
      e.preventDefault();
      const list = [...document.querySelectorAll('#sessions .sess-row .sess')];
      const at = list.findIndex(b => b.classList.contains('on'));
      const next = list[(at + ((e.key === ']' || e.key === '}') ? 1 : -1) + list.length) % list.length];
      next?.click();
    }
    if (e.key === 'Escape' && !$('#settings').hidden && !document.querySelector('dialog[open]')) closeSettings();
  });
  setInterval(renderSessions, 60000);  // keep relative times fresh
}

wire();
setDetail(S.detail);
if (S.drawer) showDrawer();
load().then(connect).catch(fail);
