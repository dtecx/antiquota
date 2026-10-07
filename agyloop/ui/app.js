'use strict';

/* agyloop GUI: sessions on the left, the relay of accounts below them, one
   transcript in the middle, a composer docked at the bottom. Everything live
   arrives over one EventSource; everything else is a small JSON call. */

const TOKEN = document.querySelector('meta[name="agyloop-token"]').content;
const $ = (s, el = document) => el.querySelector(s);
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
  S.drawer = localStorage.getItem('agyloop.drawer') === '1';
  S.tab = localStorage.getItem('agyloop.tab') || 'accounts';
  S.detail = localStorage.getItem('agyloop.detail') || 'detailed';
} catch {}
function remember(k, v) { try { localStorage.setItem('agyloop.' + k, v); } catch {} }

async function api(method, path, body) {
  const r = await fetch('/api/' + path, {
    method,
    headers: { 'X-Agyloop-Token': TOKEN, ...(body ? { 'Content-Type': 'application/json' } : {}) },
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
      if (/^file:\/\//i.test(url)) return `<span class="path" title="${url.replace(/^file:\/\//i, '')}">${text}</span>`;
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
      onclick: () => navigator.clipboard.writeText($('code', pre).textContent).then(() => toast('Copied')),
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
function renderSessions() {
  const q = $('#search').value.trim().toLowerCase();
  const list = [...S.sessions.values()]
    .filter(m => !q || (m.title || '').toLowerCase().includes(q) || m.cwd.toLowerCase().includes(q))
    .sort((a, b) => b.updated - a.updated);
  const groups = new Map();
  for (const m of list) {
    if (!groups.has(m.cwd)) groups.set(m.cwd, []);
    groups.get(m.cwd).push(m);
  }
  const nav = $('#sessions');
  nav.replaceChildren();
  if (!list.length) nav.append(h('div', { class: 'relay-empty' }, q ? 'No match.' : 'No sessions yet.'));
  for (const [cwd, ms] of groups) {
    nav.append(h('div', { class: 'group' },
      h('div', { class: 'group-name', title: cwd }, cwd.split('/').pop() || cwd),
      ms.map(m => {
        const g = m.goal;
        const goalOn = g && g.state === 'active';
        let sub = '';
        if (m.status === 'sleeping') sub = 'sleeping until ' + clock(m.status_until);
        else if (g) sub = `◎ ${g.state === 'active' ? 'round ' + g.round + '/' + g.max_rounds : g.state}`;
        return h('button', {
          class: 'sess' + (m.id === S.cur ? ' on' : ''), title: m.title || 'Untitled',
          onclick: () => open(m.id),
        },
        h('span', { class: `dot ${m.status}${goalOn ? ' goal' : ''}` }),
        h('span', { class: 't' }, m.title || 'Untitled'),
        h('span', { class: 'when' }, ago(m.updated)),
        sub && h('span', { class: 'sub' }, sub));
      })));
  }
}

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
      : h('div', { class: 'acct', title: acct.busy ? acct.slot + ' is running a call' : acct.slot },
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
  running: 'working', waiting: 'waiting for an account', checking: 'running check', verifying: 'verifier reviewing',
};
function renderHeader() {
  const m = cur();
  const busy = m && m.status !== 'idle';
  $('#title').textContent = m ? (m.title || 'Untitled') : 'agyloop';
  $('#cwd').textContent = m ? '\u200e' + m.cwd.replace(/^\/Users\/[^/]+/, '~') + '\u200e' : '';
  $('#cwd').title = m ? m.cwd : '';
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
  document.title = (busy ? '● ' : '') + (m ? (m.title || 'Untitled') + ' · ' : '') + 'agyloop';
  renderGoalbar();
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
    'agy-loop signs in its own Google accounts, separate from your agy CLI. Add at least one to start.',
    h('div', { class: 'acts' }, h('button', { class: 'primary btn small', onclick: () => openAcct() }, 'Add Google account')));
}
function blankFor(m) {
  return h('div', { class: 'blank' },
    needAccount(),
    h('h2', {}, 'What should agy work on?'),
    h('p', {}, `Runs headless in ${m.cwd.replace(/^\/Users\/[^/]+/, '~')} on the first account with quota left. `
      + 'Goal mode keeps the agent going until it is done and your checks agree, passing the work between accounts as quota runs out.'),
    h('div', { class: 'row' }, h('button', { class: 'btn', onclick: openGoal }, '◎ Set a goal')),
    h('div', { class: 'keys' },
      h('kbd', {}, 'Enter'), h('span', {}, 'send'),
      h('kbd', {}, '⇧ Enter'), h('span', {}, 'new line'),
      h('kbd', {}, '/'), h('span', {}, 'commands'),
      h('kbd', {}, 'Esc'), h('span', {}, 'stop the running turn'),
      h('kbd', {}, '⌘N'), h('span', {}, 'new session')));
}
function renderBlank() {
  $('#transcript').replaceChildren(h('div', { class: 'blank' },
    needAccount(),
    h('h2', {}, 'Start a session'),
    h('p', {}, 'Pick a working folder and a model. Every account you sign in with agyloop shares one stack of skills, hooks and MCP servers.'),
    h('div', { class: 'row' }, h('button', { class: 'primary', onclick: openNew }, 'New session'))));
  renderHeader();
}

const WHO = { user: 'you', goal: 'goal', nudge: 'goal push' };
function userNode(ev) {
  // A goal prompt carries standing instructions; the objective is what matters to read.
  const text = ev.kind === 'goal' ? ev.text.split('\n\n')[0].replace(/^Objective:\s*/, '') : ev.text;
  return h('div', { class: 'msg-user ' + (ev.kind || 'user'), title: ev.kind === 'goal' ? ev.text : null },
    h('span', { class: 'who' }, WHO[ev.kind] || 'you'), text, attachmentChips(ev, S.cur));
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
  if (!confirm(`Put every file the agent changed in turn ${turn} and later back as it was before turn ${turn}?`)) return;
  try {
    const r = await api('POST', `sessions/${m.id}/rewind`, { turn });
    toast(`Rewound ${r.files.length} file${r.files.length > 1 ? 's' : ''}`);
  } catch (e) { fail(e); }
}
function copyButton(getText) {
  return h('button', {
    class: 'copy-msg', type: 'button', title: 'Copy', 'aria-label': 'Copy message',
    onclick: e => { e.stopPropagation(); navigator.clipboard.writeText(getText()).then(() => toast('Copied')); },
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
      if (live && ev.status !== 'quota') notify(`${cur()?.title || 'agyloop'}: turn ${ev.turn} ${ev.status === 'SUCCESS' ? 'done' : ev.status}`, `${ev.slot} · ${secs(ev.secs)}`);
      $$('.turn.live', tr).forEach(n => n.classList.remove('live'));
      for (const [k, st] of S.live) { st.node.classList.remove('live'); S.live.delete(k); }
      tr.append(turnEnd(ev));
      break;
    case 'notice':
      if (live && ev.kind === 'goal' && (ev.data || {}).met) notify('Goal met', cur()?.title || '');
      if (live && ev.kind === 'wall') notify('Out of quota', ev.text);
      tr.append(noticeNode(ev));
      break;
  }
  if (stick) scrollEnd();
}

async function open(sid) {
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
  $('#side').classList.remove('open');
  $('#input').focus();
}

/* ---------- live events ---------- */
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
      next ? open(next.id) : renderBlank();
    }
    return;
  }
  if (m.sid === S.cur) {
    if (m.t === 'tool' && m.state !== 'running') {
      S.events.push(m);
      if (S.drawer && S.tab === 'changes') renderPane();
    }
    renderEvent(m, true);
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
  HIST.list.push(text);
  HIST.pos = -1;
  if ('Notification' in window && Notification.permission === 'default') Notification.requestPermission().catch(() => {});
  t.value = '';
  autosize();
  syncSend();
  try {
    const r = await api('POST', `sessions/${m.id}/send`, { text, attachments: ready.map(f => f.path) });
    ATT.length = 0;
    renderTray();
    if (r.result === 'queued') toast('Queued after the current turn');
  } catch (e) { t.value = text; fail(e); }
}
function stop() {
  if (S.cur) api('POST', `sessions/${S.cur}/stop`).then(() => toast('Stopped')).catch(fail);
}
const patch = fields => S.cur && api('PATCH', 'sessions/' + S.cur, fields).catch(fail);

/* ---------- slash commands ---------- */
const SLASH = [
  ['/goal', 'set a goal; empty opens the goal panel'],
  ['/clear', 'start a fresh agy conversation (the transcript stays)'],
  ['/compact', 'fresh conversation that starts from a summary of this one'],
  ['/rewind', 'put files back as they were before a turn: /rewind <turn>'],
  ['/export', 'download this transcript as markdown'],
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
  ['/permissions', 'edit agy-loop allow rules'],
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
        headers: { 'X-Agyloop-Token': TOKEN, 'X-Filename': encodeURIComponent(slot.name) },
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
function notify(title, body) {
  if (!document.hidden || !('Notification' in window) || Notification.permission !== 'granted') return;
  try { new Notification(title, { body, tag: 'agyloop' }); } catch {}
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
  const a = h('a', { href: URL.createObjectURL(new Blob([out.join('\n')], { type: 'text/markdown' })),
    download: (m.title || 'session').replace(/[^\w.-]+/g, '-').slice(0, 60) + '.md' });
  document.body.append(a);
  a.click();
  a.remove();
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
      case '/clear': need(); await api('POST', `sessions/${m.id}/clear`); return toast('Fresh conversation from the next message');
      case '/compact': need(); await api('POST', `sessions/${m.id}/compact`); return toast('Compacted');
      case '/rewind': return rewindTo(+arg || 0);
      case '/export': return exportTranscript();
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
        if (confirm(`Delete "${m.title || 'Untitled'}" and its transcript?`)) await api('DELETE', 'sessions/' + m.id);
        return;
      default:
        toast('Unknown command ' + cmd);
    }
  } catch (e) { fail(e); }
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
      h('b', {}, email), ', finish the steps, then check again. agy-loop also re-checks it by itself every 10 minutes.'),
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
function winLabel(w) { return w === '5h' ? '5 hour' : w; }
async function renderPane() {
  const pane = $('#pane');
  if (S.tab === 'accounts') {
    pane.replaceChildren(...[
      h('p', { class: 'cap' }, 'Accounts are used in order: one drains before the next takes over. Gemini and Claude/GPT quotas run out separately; a spent pool is walled until its reset.'),
      !S.accounts.length && h('p', { class: 'cap' }, 'No account yet. Add one: agy-loop signs it in separately from your agy CLI.'),
      ...S.accounts.map(acct => h('div', { class: 'card' + (acct.verify ? ' verify' : '') },
        h('div', { class: 'card-head' }, h('b', {}, acct.slot),
          h('span', {}, (acct.busy ? 'running · ' : '') + (acct.probed ? 'probed ' + ago(acct.probed) : 'not probed'))),
        acct.verify && verifyBlock(acct.slot, acct.verify),
        acct.today && acct.today.turns ? h('div', { class: 'today', title: 'Tokens this account used today through agy-loop' },
          `today  ↑ ${fmtK(acct.today.input)}  ↓ ${fmtK(acct.today.output)}` + (acct.today.thinking ? `  ✻ ${fmtK(acct.today.thinking)}` : '') +
          `  · ${acct.today.turns} turn${acct.today.turns > 1 ? 's' : ''}`) : null,
        !acct.verify && Object.entries({ gemini: 'Gemini', cg: 'Claude / GPT' }).map(([b, label]) => h('div', { class: 'pool' },
          h('div', { class: 'pool-name' }, h('span', {}, label), acct.walls[b] ? h('em', {}, 'walled until ' + clock(acct.walls[b])) : null),
          Object.entries(acct.quota[b] || {}).map(([w, [frac, reset]]) => h('div', { class: 'win', title: reset ? 'resets ' + new Date(reset * 1000).toLocaleString() : 'disabled until the other window resets' },
            h('span', {}, winLabel(w)),
            h('div', { class: 'bar' + (frac <= .02 ? ' wall' : frac < .2 ? ' low' : '') }, h('i', { style: `width:${Math.max(2, frac * 100)}%` })),
            h('span', { class: 'pct' }, Math.round(frac * 100) + '%'))))),
        h('div', { class: 'card-acts' },
          h('button', { class: 'btn small', title: 'Sign in with the same Google account again (expired token)', onclick: () => openAcct() }, 'Sign in again'),
          h('button', {
            class: 'btn small', onclick: () => confirm(`Remove ${acct.slot} from agy-loop? Your agy CLI keeps its own sign-in. You can add it again later.`)
              && api('DELETE', 'accounts/' + encodeURIComponent(acct.slot)).then(() => toast('Removed ' + acct.slot)).catch(fail),
          }, 'Remove')))),
      h('div', { class: 'add-row' },
        h('button', { class: 'primary', onclick: () => openAcct() }, 'Add account'),
        h('button', { class: 'btn', onclick: () => api('POST', 'accounts/probe').then(() => toast('Probing accounts…')).catch(fail) }, 'Probe now')),
    ].filter(Boolean));
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
    pane.replaceChildren(
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
      h('p', { class: 'cap' }, 'agy-loop runs on its own copy of your agy setup. Every account loads it; your agy CLI and the IDE are never changed.'),
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
  $('#new-cwd').value = m ? m.cwd : S.defaults.cwd;
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
    });
    S.sessions.set(meta.id, meta);
    await open(meta.id);
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
  try { w = window.open('', '_blank'); if (w) w.opener = null; } catch { w = null; }
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
    const text = (await navigator.clipboard.readText()).trim();
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

/* ---------- settings: agy-loop's own copy of your agy setup ---------- */
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
      onclick: () => confirm(`Delete ${kind} "${r.name}" from agy-loop? Your agy CLI and the IDE keep theirs.`)
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
function defaultsForm(df) {
  const fam = h('select', { id: 'df-family' }), eff = h('select', { id: 'df-effort' });
  const mode = h('select', { id: 'df-mode' }, [['default', 'default'], ['accept-edits', 'accept edits'], ['plan', 'plan'], ['bypass', 'bypass']]
    .map(([v, t]) => h('option', { value: v }, t)));
  const ver = h('select', { id: 'df-vmodel' });
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
    h('div', { class: 'add-row' }, h('button', {
      class: 'primary', onclick: async () => {
        try {
          S.defaults = { ...S.defaults, ...await api('PUT', 'settings/defaults', { model: eff.value, mode: mode.value, verifier_model: ver.value }) };
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
    head('General', 'Defaults for new sessions, and the copy of your agy setup that agy-loop runs on.'),
    defaultsForm(d.defaults),
    h('div', { class: 'set-block' },
      h('h3', {}, 'agy-loop stack'),
      h('p', { class: 'cap' }, `Every agy-loop account runs with ${d.path.replace(/^\/Users\/[^/]+/, '~')}: your rules, skills, hooks, MCP servers, plugins and permissions, copied. Edits here never touch ~/.gemini (your agy CLI) or the Antigravity IDE, and theirs only arrive when you copy them.`),
      h('div', { class: 'add-row' }, h('button', {
        class: 'btn', onclick: () => confirm("Replace agy-loop's whole config with a fresh copy of your agy CLI setup? Edits made here are lost; conversations stay.") && doCopy('cli', 'everything'),
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
    head('Permissions', 'Headless agy cannot ask: anything not allowed is denied and ends the turn. These rules apply to every agy-loop account, not to your agy CLI.',
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
  el.replaceChildren(
    h('span', { class: 'ctx' + (pct >= 80 ? ' hot' : pct >= 50 ? ' warm' : ''), title: `Context sent with the last request: ${ctx.toLocaleString()} of ${win.toLocaleString()} tokens` },
      h('span', { class: 'ctx-bar' }, h('i', { style: `width:${Math.max(pct, ctx ? 2 : 0)}%` })),
      ctx ? `${fmtK(ctx)} / ${fmtK(win)} context` : 'no context yet'),
    h('span', { title: 'Tokens this session: sent, received, thinking' },
      `↑ ${fmtK(m.usage.input)}  ↓ ${fmtK(m.usage.output)}` + (m.usage.thinking ? `  ✻ ${fmtK(m.usage.thinking)}` : '')),
    acct && h('span', { class: 'acct-q' + (left != null && left < 20 ? ' low' : ''), title: `${acct.slot}: ${POOL_LABEL[pool]} left (lowest window)` },
      `${who(acct.slot)} · ${pool === 'cg' ? 'claude/gpt' : 'gemini'} ${left == null ? '–' : left + '%'}`),
    h('span', { class: 'grow' }),
    h('span', { class: 'hint' }, S.accounts.filter(x => !x.walls[pool] && !x.verify).length + ' of ' + S.accounts.length + ' accounts free'));
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
  if (want && S.sessions.has(want)) await open(want);
  else if (st.sessions.length) await open(st.sessions[0].id);
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
  $('#mode').addEventListener('change', e => {
    if (e.target.value === 'bypass' && !confirm('Bypass lets the agent run any command and edit any file without asking. Continue?')) {
      e.target.value = cur()?.mode || 'default';
      return;
    }
    patch({ mode: e.target.value });
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

  document.addEventListener('keydown', e => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === 'n') { e.preventDefault(); openNew(); }
    if (mod && e.key === '.') { e.preventDefault(); stop(); }
    if (mod && e.key === ',') { e.preventDefault(); openSettings(); }
    if (e.key === 'Escape' && !$('#settings').hidden && !document.querySelector('dialog[open]')) closeSettings();
  });
  setInterval(renderSessions, 60000);  // keep relative times fresh
}

wire();
setDetail(S.detail);
if (S.drawer) showDrawer();
load().then(connect).catch(fail);
