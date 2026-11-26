// Console socle — application monopage sans dépendance.
// Sécurité : le DOM est construit exclusivement via createElement/textContent
// (aucun innerHTML de données), le jeton reste dans sessionStorage.

const store = {
  get(k) { try { return sessionStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { sessionStorage.setItem(k, v); } catch { /* stockage indisponible */ } },
  del(k) { try { sessionStorage.removeItem(k); } catch { /* idem */ } },
};

const state = { token: store.get('socle.token'), me: null, spec: null, timers: [] };

// ---------------------------------------------------------------- utilitaires
function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === null || v === undefined || v === false) continue;
    if (k === 'class') el.className = v;
    // CSSOM (autorisé par la CSP), contrairement à l'attribut style en ligne.
    else if (k === 'style') el.style.cssText = v;
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'value') el.value = v;
    else el.setAttribute(k, v === true ? '' : String(v));
  }
  for (const c of children.flat(Infinity)) {
    if (c === null || c === undefined || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

const fmtDate = (s) => {
  if (!s) return '—';
  const d = new Date(s);
  return isNaN(d) ? s : d.toLocaleString('fr-FR', { dateStyle: 'short', timeStyle: 'short' });
};
const ago = (s) => {
  if (!s) return 'jamais';
  const secs = Math.round((Date.now() - new Date(s).getTime()) / 1000);
  if (secs < 60) return `il y a ${secs} s`;
  if (secs < 3600) return `il y a ${Math.round(secs / 60)} min`;
  if (secs < 86400) return `il y a ${Math.round(secs / 3600)} h`;
  return `il y a ${Math.round(secs / 86400)} j`;
};
const short = (s, n = 12) => (s ? String(s).slice(0, n) : '—');

const STATUS = {
  awaiting_approval: ['En attente d’approbation', 'warn'],
  queued: ['Planifiée', 'accent'],
  running: ['En cours', 'accent'],
  succeeded: ['Réussie', 'ok'],
  failed: ['Échec', 'bad'],
  rolled_back: ['Retour arrière', 'warn'],
  rejected: ['Refusée', 'bad'],
  cancelled: ['Annulée', ''],
};
const KIND = {
  deploy: 'Déploiement', rollback: 'Retour arrière', scale: 'Mise à l’échelle', infra_plan: 'Plan d’infrastructure',
  infra_apply: 'Application d’infrastructure', backup: 'Sauvegarde', restore: 'Restauration',
  restore_drill: 'Exercice de restauration', patch: 'Correctifs', runbook: 'Runbook',
};
const INC_STATUS = {
  open: ['Ouvert', 'bad'], acknowledged: ['Pris en charge', 'warn'], mitigated: ['Atténué', 'accent'],
  resolved: ['Résolu', 'ok'], closed: ['Clos', ''],
};
const statusPill = (s) => { const [l, c] = STATUS[s] || [s, '']; return h('span', { class: `pill ${c}` }, l); };
const sevPill = (s) => h('span', { class: `pill ${s === 'sev1' || s === 'sev2' ? 'bad' : s === 'sev3' ? 'warn' : ''}` }, String(s).toUpperCase());
const incPill = (s) => { const [l, c] = INC_STATUS[s] || [s, '']; return h('span', { class: `pill ${c}` }, l); };

function toast(message, error = false) {
  const t = h('div', { class: `toast${error ? ' error' : ''}`, role: 'status' }, message);
  document.body.append(t);
  setTimeout(() => t.remove(), error ? 7000 : 3500);
}

// ----------------------------------------------------------------------- API
async function api(method, path, body) {
  const headers = { Authorization: `Bearer ${state.token}` };
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  const res = await fetch(`/api/v1${path}`, { method, headers, body: body !== undefined ? JSON.stringify(body) : undefined });
  if (res.status === 401) { logout(); throw new Error('Session expirée ou jeton invalide'); }
  const type = res.headers.get('content-type') || '';
  const data = type.includes('json') ? await res.json().catch(() => null) : await res.text();
  if (!res.ok) throw new Error(data?.error?.message || `Erreur HTTP ${res.status}`);
  return data;
}
const get = (p) => api('GET', p);
const post = (p, b = {}) => api('POST', p, b);

async function download(path, filename) {
  const res = await fetch(`/api/v1${path}`, { headers: { Authorization: `Bearer ${state.token}` } });
  if (!res.ok) { toast(`Téléchargement impossible (HTTP ${res.status})`, true); return; }
  const url = URL.createObjectURL(await res.blob());
  const a = h('a', { href: url, download: filename });
  document.body.append(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// ------------------------------------------------------------- application
function logout() {
  store.del('socle.token');
  state.token = null; state.me = null;
  renderLogin();
}

function renderLogin(error) {
  const app = document.getElementById('app');
  const input = h('input', { type: 'password', id: 'tok', autocomplete: 'off', placeholder: 'socle_pat_…', required: true });
  app.replaceChildren(h('div', { class: 'login' }, h('div', { class: 'card' },
    h('div', { class: 'brand' }, h('span', { class: 'brand-mark' }, 's'), 'socle'),
    h('h2', {}, 'Connexion à la console'),
    h('p', { class: 'subtitle' }, 'Collez un jeton d’API, ou passez par le proxy SSO de votre organisation (OIDC).'),
    error ? h('div', { class: 'banner bad' }, error) : null,
    h('form', {
      onsubmit: async (e) => {
        e.preventDefault();
        state.token = input.value.trim();
        try { await boot(); store.set('socle.token', state.token); } catch (err) { state.token = null; renderLogin(err.message); }
      },
    }, h('label', { for: 'tok' }, 'Jeton'), input, h('div', { class: 'actions', style: 'margin-top:14px' }, h('button', { class: 'btn primary', type: 'submit' }, 'Se connecter'))),
  )));
  input.focus();
}

const NAV = [
  ['Pilotage', [['#/', 'Vue d’ensemble'], ['#/operations', 'Opérations', 'pending'], ['#/operations/new', 'Nouvelle opération'], ['#/environments', 'Environnements'], ['#/releases', 'Releases']]],
  ['Exploitation', [['#/incidents', 'Incidents', 'incidents'], ['#/backups', 'Sauvegardes'], ['#/maintenance', 'Maintenance']]],
  ['Gouvernance', [['#/audit', 'Audit'], ['#/compliance', 'Conformité'], ['#/tokens', 'Jetons d’API', null, true]]],
];

function shell(content, counts = {}) {
  const hash = location.hash || '#/';
  const nav = h('nav', { class: 'nav', 'aria-label': 'Navigation principale' });
  for (const [section, items] of NAV) {
    nav.append(h('div', { class: 'section' }, section));
    for (const [href, label, counter, adminOnly] of items) {
      if (adminOnly && !state.me?.is_admin) continue;
      const active = href === '#/' ? hash === '#/' || hash === '#' : hash === href || (hash.startsWith(`${href}/`) && href !== '#/operations');
      const n = counter ? counts[counter] : 0;
      nav.append(h('a', { href, class: active ? 'active' : '' }, label, n ? h('span', { class: 'count' }, n) : null));
    }
  }
  const me = state.me?.principal;
  return h('div', { class: 'shell' },
    h('aside', { class: 'sidebar' }, h('div', { class: 'brand' }, h('span', { class: 'brand-mark' }, 's'), 'socle'), nav),
    h('div', { class: 'main' },
      h('header', { class: 'topbar' },
        h('div', { class: 'subtitle' }, state.spec ? `${state.spec.platform.name} · spécification ${short(state.spec.hash)}` : ''),
        h('div', { class: 'who' }, me ? `${me.display_name}` : '', h('button', { class: 'btn small', onclick: logout }, 'Déconnexion'))),
      h('main', { class: 'content', id: 'content' }, content)));
}

async function boot() {
  state.me = await get('/whoami');
  state.spec = await get('/spec');
  window.addEventListener('hashchange', route);
  await route();
}

function clearTimers() { state.timers.forEach(clearInterval); state.timers = []; }
function every(ms, fn) { state.timers.push(setInterval(fn, ms)); }

const ROUTES = [
  [/^#?\/?$/, viewOverview],
  [/^#\/operations(?:\?.*)?$/, viewOperations],
  [/^#\/operations\/new$/, viewNewOperation],
  [/^#\/operations\/([\w-]+)$/, viewOperation],
  [/^#\/environments$/, viewEnvironments],
  [/^#\/releases$/, viewReleases],
  [/^#\/incidents$/, viewIncidents],
  [/^#\/incidents\/([\w-]+)$/, viewIncident],
  [/^#\/backups$/, viewBackups],
  [/^#\/maintenance$/, viewMaintenance],
  [/^#\/audit$/, viewAudit],
  [/^#\/compliance$/, viewCompliance],
  [/^#\/tokens$/, viewTokens],
];

async function route() {
  clearTimers();
  const hash = location.hash || '#/';
  const app = document.getElementById('app');
  const match = ROUTES.map(([re, fn]) => [hash.match(re), fn]).find(([m]) => m);
  const counts = await get('/overview').then((o) => ({ pending: o.awaiting_approval.length, incidents: o.open_incidents.length })).catch(() => ({}));
  const placeholder = h('div', { class: 'empty' }, 'Chargement…');
  app.replaceChildren(shell(placeholder, counts));
  try {
    const view = match ? await match[1](...match[0].slice(1)) : h('div', { class: 'card' }, 'Page introuvable.');
    document.getElementById('content').replaceChildren(view);
  } catch (err) {
    document.getElementById('content').replaceChildren(h('div', { class: 'banner bad' }, err.message));
  }
}

const refresh = () => route();
const card = (title, ...children) => h('section', { class: 'card' }, title ? h('h2', {}, title) : null, ...children);
const pageHead = (title, subtitle, ...actions) => h('div', { class: 'page-head' }, h('div', {}, h('h1', {}, title), subtitle ? h('div', { class: 'subtitle' }, subtitle) : null), h('div', { class: 'actions' }, ...actions));

function tableOf(headers, rows, onClick) {
  if (!rows.length) return h('div', { class: 'empty' }, 'Aucun élément.');
  return h('div', { class: 'table-wrap' }, h('table', {},
    h('thead', {}, h('tr', {}, headers.map((x) => h('th', {}, x)))),
    h('tbody', {}, rows.map(([key, cells]) => h('tr', { class: onClick ? 'clickable' : '', onclick: onClick ? () => onClick(key) : null }, cells.map((c) => h('td', {}, c)))))));
}

function opRows(ops) {
  return ops.map((o) => [o.id, [
    statusPill(o.status), KIND[o.request.action.kind] || o.request.action.kind, o.request.environment,
    o.request.action.service || o.request.action.stack || o.request.action.policy || o.request.action.target || o.request.action.runbook || '—',
    o.requested_by, fmtDate(o.created_at)]]);
}
const OP_HEAD = ['Statut', 'Type', 'Env.', 'Objet', 'Demandeur', 'Créée'];

// ----------------------------------------------------------- vue d'ensemble
async function viewOverview() {
  const o = await get('/overview');
  every(10000, async () => { if ((location.hash || '#/') === '#/') refresh(); });
  const compliant = o.backups.filter((b) => b.rpo_compliant && (!b.drill || (!b.drill.overdue && b.drill.within_rto !== false))).length;
  const kpi = (label, value, cls) => h('div', { class: `card kpi ${cls || ''}` }, h('div', { class: 'label' }, label), h('div', { class: 'value' }, value));
  return h('div', {},
    pageHead('Vue d’ensemble', `${o.platform} — état en temps réel`, h('a', { class: 'btn primary', href: '#/operations/new' }, 'Nouvelle opération')),
    h('div', { class: 'grid kpis' },
      kpi('Approbations en attente', o.awaiting_approval.length, o.awaiting_approval.length ? 'warn' : ''),
      kpi('Opérations en cours', o.running.length),
      kpi('Incidents actifs', o.open_incidents.length, o.open_incidents.length ? 'bad' : 'ok'),
      kpi('Sauvegardes conformes', `${compliant}/${o.backups.length}`, compliant === o.backups.length ? 'ok' : 'bad')),
    o.awaiting_approval.length ? card('À approuver', tableOf(OP_HEAD, opRows(o.awaiting_approval), (id) => { location.hash = `#/operations/${id}`; })) : null,
    o.open_incidents.length ? card('Incidents actifs', tableOf(['Gravité', 'Statut', 'Titre', 'Env.', 'Ouvert'],
      o.open_incidents.map((i) => [i.id, [sevPill(i.severity), incPill(i.status), i.title, i.environment || '—', ago(i.created_at)]]), (id) => { location.hash = `#/incidents/${id}`; })) : null,
    h('div', { class: 'grid two' }, o.environments.map(envCard)),
    card('Activité récente', tableOf(OP_HEAD, opRows(o.recent), (id) => { location.hash = `#/operations/${id}`; })));
}

function envCard(env) {
  const badges = [h('span', { class: `pill ${env.tier === 'production' ? 'bad' : env.tier === 'staging' ? 'warn' : 'accent'}` }, env.tier)];
  badges.push(env.change_window_open ? h('span', { class: 'pill ok' }, 'fenêtre ouverte') : h('span', { class: 'pill warn', title: env.next_window ? `Prochaine : ${fmtDate(env.next_window)}` : '' }, 'hors fenêtre'));
  if (env.freeze) badges.push(h('span', { class: 'pill bad' }, `gel : ${env.freeze}`));
  if (env.maintenance) badges.push(h('span', { class: 'pill warn' }, 'maintenance'));
  return h('section', { class: 'card env-card' },
    h('div', { class: 'env-head' }, h('h2', { style: 'margin:0' }, env.name), h('div', { class: 'badges' }, badges)),
    env.services.length ? env.services.map((s) => h('div', { class: 'svc' },
      h('div', {}, h('strong', {}, s.name), h('div', { class: 'meta' }, s.team)),
      h('div', {}, s.version || h('span', { class: 'meta' }, 'non déployé'), s.image_ref ? h('div', { class: 'meta mono', title: s.image_ref }, short(s.image_ref.split('@')[1]?.slice(7), 12)) : null),
      h('div', { class: 'meta' }, `${s.target_kind} · ${s.provider}`, h('br'), `${s.strategy}${s.deployed_at ? ` · ${ago(s.deployed_at)}` : ''}`),
      s.drift ? h('span', { class: 'pill bad', title: s.drift }, 'dérive') : h('span', { class: 'pill ok' }, 'conforme'))) : h('div', { class: 'empty' }, 'Aucun service.'));
}

// --------------------------------------------------------------- opérations
async function viewOperations() {
  const params = new URLSearchParams(location.hash.split('?')[1] || '');
  const envSel = h('select', {}, h('option', { value: '' }, 'Tous'), Object.keys(state.spec.environments).map((e) => h('option', { value: e }, e)));
  const stSel = h('select', {}, h('option', { value: '' }, 'Tous'), Object.entries(STATUS).map(([k, [l]]) => h('option', { value: k }, l)));
  const kindSel = h('select', {}, h('option', { value: '' }, 'Tous'), Object.entries(KIND).map(([k, l]) => h('option', { value: k }, l)));
  envSel.value = params.get('environment') || ''; stSel.value = params.get('status') || ''; kindSel.value = params.get('kind') || '';
  const list = h('div', {});
  const load = async () => {
    const q = new URLSearchParams({ limit: '100' });
    if (envSel.value) q.set('environment', envSel.value);
    if (stSel.value) q.set('status', stSel.value);
    if (kindSel.value) q.set('kind', kindSel.value);
    const ops = await get(`/operations?${q}`);
    list.replaceChildren(tableOf(OP_HEAD, opRows(ops), (id) => { location.hash = `#/operations/${id}`; }));
  };
  [envSel, stSel, kindSel].forEach((s) => s.addEventListener('change', load));
  await load();
  every(8000, load);
  return h('div', {},
    pageHead('Opérations', 'Tout changement passe par le même circuit : portes → approbation → exécution → vérification → retour arrière.', h('a', { class: 'btn primary', href: '#/operations/new' }, 'Nouvelle opération')),
    card(null, h('form', { onsubmit: (e) => e.preventDefault() }, h('div', { class: 'row' },
      h('div', {}, h('label', {}, 'Environnement'), envSel), h('div', {}, h('label', {}, 'Statut'), stSel), h('div', {}, h('label', {}, 'Type'), kindSel))), list));
}

function gateList(gates) {
  if (!gates?.length) return h('div', { class: 'empty' }, 'Aucune porte applicable.');
  const icon = { passed: ['✓', 'icon-ok'], failed: ['✗', 'icon-bad'], warning: ['!', 'icon-warn'], overridden: ['⚠', 'icon-warn'] };
  return h('ul', { class: 'gates' }, gates.map((g) => {
    const [i, c] = icon[g.status] || ['?', ''];
    return h('li', {}, h('span', { class: c }, i), h('strong', {}, g.gate), h('span', {}, g.message));
  }));
}

function planSteps(plan) {
  const block = (title, steps) => (steps?.length ? h('div', { style: 'margin-top:10px' }, h('h3', {}, title),
    h('ol', { class: 'steps' }, steps.map((s) => h('li', {}, h('div', {}, s.name), h('div', { class: 'desc' }, describe(s.action)))))) : null);
  return h('div', {}, block('Exécution', plan.steps), block('Vérification', plan.verify), block('Retour arrière (si échec)', plan.rollback),
    plan.locks?.length ? h('div', { class: 'hint' }, `Verrous : ${plan.locks.join(', ')}`) : null);
}

function describe(a) {
  switch (a.type) {
    case 'exec': return [a.program, ...a.args].join(' ');
    case 'gitops_update': return a.edits.map((e) => (e.op === 'set_image' ? `${e.file} → ${e.new_name}@${e.digest}` : `${e.file} (réécrit)`)).join(', ') + (a.push ? ` ; push ${a.branch}` : ' ; commit');
    case 'wait_argo': return `Argo CD ${a.application} (${a.context}) : Synced + Healthy${a.image_ref ? `, image ${a.image_ref}` : ''}`;
    case 'http_check': return `GET ${a.url} → ${a.expect_status} (${a.retries} essais)`;
    case 'prom_check': return `PromQL ${a.query} ≤ ${a.max}`;
    case 'tf_plan_guard': return a.expected ? `plan identique au plan approuvé (${short(a.expected)})` : `empreinte du plan ${a.chdir}`;
    case 'silence': return `silence Alertmanager ${Object.entries(a.matchers).map(([k, v]) => `${k}=${v}`).join(',')}`;
    default: return JSON.stringify(a);
  }
}

async function viewOperation(id) {
  const op = await get(`/operations/${id}`);
  const logs = h('div', { class: 'logs', role: 'log' });
  let last = 0;
  const loadLogs = async () => {
    const lines = await get(`/operations/${id}/logs?after=${last}`);
    for (const l of lines) {
      last = l.seq;
      logs.append(h('div', { class: `l-${l.level}` }, h('span', { class: 'ts' }, `${new Date(l.ts).toLocaleTimeString('fr-FR')} `), `${l.step} │ ${l.message}`));
    }
    if (lines.length) logs.scrollTop = logs.scrollHeight;
  };
  await loadLogs();
  if (!['succeeded', 'failed', 'rolled_back', 'rejected', 'cancelled'].includes(op.status)) {
    every(2500, async () => {
      await loadLogs();
      const fresh = await get(`/operations/${id}`);
      if (fresh.status !== op.status) refresh();
    });
  }
  const comment = h('textarea', { placeholder: 'Commentaire (obligatoire pour un refus)' });
  const act = async (action, body) => {
    try { await post(`/operations/${id}/${action}`, body); toast('Décision enregistrée.'); refresh(); } catch (e) { toast(e.message, true); }
  };
  const me = state.me?.principal?.id;
  const canDecide = op.status === 'awaiting_approval' && op.requested_by !== me;
  const granted = op.approvals.filter((a) => a.decision === 'approved' && a.plan_hash === op.plan_hash).length;
  const a = op.request.action;
  return h('div', {},
    pageHead(`${KIND[a.kind] || a.kind} · ${a.service || a.stack || a.policy || a.target || a.runbook}`, op.plan.summary, statusPill(op.status)),
    op.outcome ? h('div', { class: `banner ${op.status === 'succeeded' ? '' : 'bad'}` }, op.outcome.message, op.outcome.incident_id ? h('span', {}, ' — ', h('a', { href: `#/incidents/${op.outcome.incident_id}` }, 'incident ouvert')) : null) : null,
    h('div', { class: 'grid two' },
      card('Détails', h('dl', { class: 'kv' },
        h('dt', {}, 'Identifiant'), h('dd', { class: 'mono' }, op.id),
        h('dt', {}, 'Environnement'), h('dd', {}, op.request.environment),
        h('dt', {}, 'Demandeur'), h('dd', {}, op.requested_by),
        h('dt', {}, 'Motif'), h('dd', {}, op.request.reason || '—'),
        h('dt', {}, 'Ticket'), h('dd', {}, op.request.change_ticket || '—'),
        h('dt', {}, 'Urgence'), h('dd', {}, op.request.emergency ? h('span', { class: 'pill bad' }, 'procédure d’urgence') : 'non'),
        h('dt', {}, 'Release'), h('dd', { class: 'mono' }, op.resolved.image_ref || '—'),
        h('dt', {}, 'Précédente'), h('dd', { class: 'mono' }, op.resolved.previous_image_ref || '—'),
        h('dt', {}, 'Empreinte du plan'), h('dd', { class: 'mono' }, op.plan_hash),
        h('dt', {}, 'Créée / terminée'), h('dd', {}, `${fmtDate(op.created_at)} → ${fmtDate(op.finished_at)}`))),
      card(`Portes et approbations (${granted}/${op.required_approvals})`, gateList(op.gates),
        op.approvals.length ? h('ul', { class: 'timeline', style: 'margin-top:12px' }, op.approvals.map((x) => h('li', {}, h('div', { class: 'when' }, fmtDate(x.at)), `${x.approver} : ${x.decision === 'approved' ? 'approuvé' : 'refusé'}${x.comment ? ` — ${x.comment}` : ''}`))) : null,
        op.status === 'awaiting_approval' ? h('div', { style: 'margin-top:14px' },
          canDecide ? h('div', {}, comment, h('div', { class: 'actions', style: 'margin-top:8px' },
            h('button', { class: 'btn primary', onclick: () => act('approve', { comment: comment.value }) }, 'Approuver ce plan'),
            h('button', { class: 'btn danger', onclick: () => act('reject', { comment: comment.value }) }, 'Refuser'))) : h('div', { class: 'hint' }, 'Principe des quatre yeux : une autre personne doit approuver.'),
          op.requested_by === me || state.me?.is_admin ? h('div', { class: 'actions', style: 'margin-top:8px' }, h('button', { class: 'btn small', onclick: () => act('cancel', { reason: 'annulée depuis la console' }) }, 'Annuler l’opération')) : null) : null)),
    card('Plan', planSteps(op.plan)),
    card('Journal d’exécution', last || op.status === 'running' ? logs : h('div', { class: 'empty' }, 'Pas encore de journal.')));
}

// ------------------------------------------------------- nouvelle opération
function viewNewOperation() {
  const spec = state.spec;
  const kind = h('select', {}, Object.entries(KIND).map(([k, l]) => h('option', { value: k }, l)));
  const fields = h('div', {});
  const preview = h('div', {});
  const reason = h('input', { placeholder: 'Pourquoi ce changement ?' });
  const ticket = h('input', { placeholder: 'CHG-1234' });
  const emergency = h('input', { type: 'checkbox' });
  let inputs = {};

  const select = (name, label, options, onchange) => {
    const s = h('select', { name }, options.map((o) => h('option', { value: o }, o)));
    if (onchange) s.addEventListener('change', onchange);
    inputs[name] = s;
    return h('div', {}, h('label', {}, label), s);
  };
  const text = (name, label, attrs = {}) => { const i = h('input', { name, ...attrs }); inputs[name] = i; return h('div', {}, h('label', {}, label), i); };

  const vmTargets = Object.entries(spec.targets).filter(([, t]) => t.kind === 'vm').map(([n]) => n);
  const build = async () => {
    inputs = {}; preview.replaceChildren();
    const k = kind.value;
    const row = h('div', { class: 'row' });
    const services = Object.keys(spec.services);
    const envsOf = (svc) => Object.keys(spec.services[svc]?.environments || {});
    if (['deploy', 'rollback', 'scale'].includes(k)) {
      const relBox = h('div', {});
      const refreshRel = async () => {
        if (k === 'scale') return;
        const rels = await get(`/releases?service=${encodeURIComponent(inputs.service.value)}&limit=30`).catch(() => []);
        const s = h('select', { name: 'release' }, k === 'rollback' ? h('option', { value: '' }, 'Release précédente (automatique)') : null, rels.map((r) => h('option', { value: r.id }, `${r.version} — ${r.digest.slice(7, 19)}`)));
        inputs.release = s;
        relBox.replaceChildren(h('label', {}, k === 'rollback' ? 'Rétablir' : 'Release'), s);
      };
      const envBox = h('div', {});
      const refreshEnv = () => { envBox.replaceChildren(select('environment', 'Environnement', envsOf(inputs.service.value))); };
      row.append(select('service', 'Service', services, () => { refreshEnv(); refreshRel(); }));
      refreshEnv();
      row.append(envBox);
      if (k === 'scale') row.append(text('min', 'Min. réplicas', { type: 'number', min: 1, value: 2 }), text('max', 'Max. réplicas', { type: 'number', min: 1, value: 6 }));
      else { row.append(relBox); await refreshRel(); }
    } else if (['infra_plan', 'infra_apply'].includes(k)) {
      const stacks = Object.keys(spec.stacks);
      const envBox = h('div', {});
      const refreshEnv = () => envBox.replaceChildren(select('environment', 'Environnement', spec.stacks[inputs.stack.value]?.environments || []));
      row.append(select('stack', 'Stack', stacks, refreshEnv)); refreshEnv(); row.append(envBox);
      if (k === 'infra_apply') row.append(text('plan_operation', 'Opération de plan approuvée', { placeholder: 'identifiant de l’opération infra_plan' }));
    } else if (['backup', 'restore_drill', 'restore'].includes(k)) {
      row.append(select('policy', 'Politique', Object.keys(spec.backups)));
      if (k === 'restore') {
        row.append(text('snapshot', 'Instantané (optionnel)'), text('point_in_time', 'Instant cible (optionnel)', { type: 'datetime-local' }));
        const ip = h('input', { type: 'checkbox', name: 'in_place' }); inputs.in_place = ip;
        row.append(h('div', {}, h('label', {}, 'Mode'), h('label', { class: 'check' }, ip, 'Restaurer en place (écrase l’existant)')));
        row.append(text('confirm', 'Confirmation : retapez l’environnement'));
      }
    } else if (k === 'patch') {
      row.append(select('target', 'Cible (VMs)', vmTargets), select('environment', 'Environnement', Object.keys(spec.environments)));
    } else if (k === 'runbook') {
      row.append(select('runbook', 'Runbook', Object.keys(spec.runbooks)), select('environment', 'Environnement', Object.keys(spec.environments)));
    }
    fields.replaceChildren(row);
  };
  kind.addEventListener('change', build);

  const request = () => {
    const k = kind.value; const v = (n) => inputs[n]?.value;
    const action = { kind: k };
    let environment = v('environment');
    if (['deploy', 'rollback', 'scale'].includes(k)) action.service = v('service');
    if (k === 'deploy') action.release = v('release');
    if (k === 'rollback' && v('release')) action.to_release = v('release');
    if (k === 'scale') { action.min = Number(v('min')); action.max = Number(v('max')); }
    if (['infra_plan', 'infra_apply'].includes(k)) action.stack = v('stack');
    if (k === 'infra_apply') action.plan_operation = v('plan_operation');
    if (['backup', 'restore_drill', 'restore'].includes(k)) { action.policy = v('policy'); environment = spec.backups[action.policy].environment; }
    if (k === 'restore') {
      if (v('snapshot')) action.snapshot = v('snapshot');
      if (v('point_in_time')) action.point_in_time = new Date(v('point_in_time')).toISOString();
      action.in_place = inputs.in_place.checked;
      if (v('confirm') !== environment) throw new Error(`Confirmation : retapez « ${environment} »`);
    }
    if (k === 'patch') action.target = v('target');
    if (k === 'runbook') action.runbook = v('runbook');
    return { environment, action, reason: reason.value, change_ticket: ticket.value || null, emergency: emergency.checked };
  };

  const doPreview = async () => {
    try {
      const p = await post('/operations/preview', request());
      const decision = { proceed: ['Exécution directe', 'ok'], await_approval: [`${p.report.required_approvals} approbation(s) requise(s)`, 'warn'], reject: ['Refusée par les portes', 'bad'] }[p.report.decision];
      preview.replaceChildren(card('Aperçu (rien n’est exécuté)', h('p', {}, p.plan.summary, ' ', h('span', { class: `pill ${decision[1]}` }, decision[0])), gateList(p.report.results), planSteps(p.plan)));
    } catch (e) { toast(e.message, true); }
  };
  const submit = async () => {
    try { const op = await post('/operations', request()); toast('Opération soumise.'); location.hash = `#/operations/${op.id}`; } catch (e) { toast(e.message, true); }
  };
  build();
  return h('div', {},
    pageHead('Nouvelle opération', 'Prévisualisez le plan et les portes avant de soumettre.'),
    card(null, h('form', { onsubmit: (e) => { e.preventDefault(); doPreview(); } },
      h('div', { class: 'row' }, h('div', {}, h('label', {}, 'Type d’opération'), kind)),
      fields,
      h('div', { class: 'row' }, h('div', {}, h('label', {}, 'Motif'), reason), h('div', {}, h('label', {}, 'Ticket de changement'), ticket)),
      h('label', { class: 'check' }, emergency, 'Procédure d’urgence (contourne les portes de processus si la politique l’autorise ; tracé et revu)'),
      h('div', { class: 'actions', style: 'margin-top:14px' }, h('button', { class: 'btn', type: 'submit' }, 'Prévisualiser'), h('button', { class: 'btn primary', type: 'button', onclick: submit }, 'Soumettre')))),
    preview);
}

// ------------------------------------------------------- environnements, releases
async function viewEnvironments() {
  const envs = await get('/environments');
  return h('div', {}, pageHead('Environnements', 'Versions en place, fenêtres de changement, gels et dérives GitOps.'),
    h('div', { class: 'grid two' }, envs.map((e) => h('div', {}, envCard(e),
      e.services.filter((s) => s.drift).map((s) => h('div', { class: 'banner bad' }, `${s.name} : ${s.drift}`))))));
}

async function viewReleases() {
  const rels = await get('/releases?limit=100');
  const yes = (v) => (v === true ? h('span', { class: 'pill ok' }, 'oui') : v === false ? h('span', { class: 'pill bad' }, 'non') : h('span', { class: 'pill' }, '—'));
  return h('div', {}, pageHead('Releases', 'Artefacts immuables (digest) et preuves de la CI : « build once, deploy many ».'),
    card(null, tableOf(['Service', 'Version', 'Digest', 'Tests', 'Vulnérabilités', 'Signée', 'SBOM', 'Créée'], rels.map((r) => [r.id, [
      r.service, r.version, h('span', { class: 'mono', title: r.digest }, r.digest.slice(7, 19)), yes(r.evidence.tests_passed),
      r.evidence.vulnerabilities ? h('span', { class: `pill ${r.evidence.vulnerabilities.critical || r.evidence.vulnerabilities.high ? 'bad' : 'ok'}` }, `C${r.evidence.vulnerabilities.critical} · H${r.evidence.vulnerabilities.high}`) : h('span', { class: 'pill' }, '—'),
      yes(r.evidence.signature_verified), r.evidence.sbom_uri ? h('span', { class: 'pill ok' }, 'oui') : h('span', { class: 'pill' }, '—'), fmtDate(r.created_at)]]))));
}

// ---------------------------------------------------------------- incidents
async function viewIncidents() {
  const list = await get('/incidents?limit=200');
  const title = h('input', { placeholder: 'Ex. : API facturation en erreur 5xx', required: true });
  const sev = h('select', {}, ['sev1', 'sev2', 'sev3', 'sev4'].map((s) => h('option', { value: s }, s.toUpperCase())));
  sev.value = 'sev3';
  const env = h('select', {}, h('option', { value: '' }, '—'), Object.keys(state.spec.environments).map((e) => h('option', { value: e }, e)));
  const summary = h('textarea', { placeholder: 'Impact observé, premiers éléments' });
  const open = async (e) => {
    e.preventDefault();
    try {
      const i = await post('/incidents', { title: title.value, severity: sev.value, environment: env.value || null, services: [], summary: summary.value });
      location.hash = `#/incidents/${i.id}`;
    } catch (err) { toast(err.message, true); }
  };
  return h('div', {}, pageHead('Incidents', 'Déclaration, coordination, chronologie et postmortem.'),
    card('Déclarer un incident', h('form', { onsubmit: open },
      h('div', { class: 'row' }, h('div', {}, h('label', {}, 'Titre'), title), h('div', {}, h('label', {}, 'Gravité'), sev), h('div', {}, h('label', {}, 'Environnement'), env)),
      h('label', {}, 'Résumé'), summary, h('div', { class: 'actions', style: 'margin-top:10px' }, h('button', { class: 'btn primary', type: 'submit' }, 'Déclarer')))),
    card('Historique', tableOf(['Gravité', 'Statut', 'Titre', 'Env.', 'Source', 'Ouvert', 'Délai de résolution'], list.map((i) => [i.id, [
      sevPill(i.severity), incPill(i.status), i.title, i.environment || '—', i.source.kind,
      fmtDate(i.created_at), i.resolved_at ? `${Math.round((new Date(i.resolved_at) - new Date(i.created_at)) / 60000)} min` : '—']]), (id) => { location.hash = `#/incidents/${id}`; })));
}

async function viewIncident(id) {
  const i = await get(`/incidents/${id}`);
  const act = async (fn) => { try { await fn(); toast('Incident mis à jour.'); refresh(); } catch (e) { toast(e.message, true); } };
  const transition = (status, label, cls = '') => h('button', { class: `btn ${cls}`, onclick: () => act(() => post(`/incidents/${id}/transition`, { status, note: note.value || null })) }, label);
  const note = h('textarea', { placeholder: 'Note de chronologie (constats, actions, décisions)' });
  const rootCause = h('textarea', { placeholder: 'Cause racine' }); rootCause.value = i.root_cause || '';
  const pmUrl = h('input', { placeholder: 'https://… (lien du postmortem)' }); pmUrl.value = i.postmortem_url || '';
  const next = {
    open: [transition('acknowledged', 'Prendre en charge', 'primary'), transition('mitigated', 'Atténué'), transition('resolved', 'Résolu')],
    acknowledged: [transition('mitigated', 'Atténué', 'primary'), transition('resolved', 'Résolu')],
    mitigated: [transition('resolved', 'Résolu', 'primary')],
    resolved: [transition('closed', 'Clore', 'primary'), transition('open', 'Rouvrir')],
    closed: [],
  }[i.status] || [];
  return h('div', {},
    pageHead(i.title, `Incident ${short(i.id, 8)} · ouvert ${ago(i.created_at)} par ${i.created_by}`, sevPill(i.severity), incPill(i.status)),
    h('div', { class: 'grid two' },
      card('Actions', note, h('div', { class: 'actions', style: 'margin-top:8px' }, next,
        h('button', { class: 'btn', onclick: () => act(() => post(`/incidents/${id}/notes`, { note: note.value })) }, 'Ajouter la note'),
        h('button', { class: 'btn', onclick: () => download(`/incidents/${id}/postmortem`, `postmortem-${id}.md`) }, 'Postmortem (Markdown)'))),
      card('Analyse', h('label', {}, 'Cause racine'), rootCause, h('label', { style: 'margin-top:8px' }, 'Postmortem'), pmUrl,
        h('div', { class: 'actions', style: 'margin-top:8px' }, h('button', { class: 'btn', onclick: () => act(() => api('PATCH', `/incidents/${id}`, { root_cause: rootCause.value || null, postmortem_url: pmUrl.value || null })) }, 'Enregistrer')),
        i.severity <= 'sev2' ? h('div', { class: 'hint' }, 'Sev1/Sev2 : la clôture exige une cause racine ou un lien de postmortem.') : null)),
    card('Contexte', h('dl', { class: 'kv' },
      h('dt', {}, 'Environnement'), h('dd', {}, i.environment || '—'), h('dt', {}, 'Services'), h('dd', {}, i.services.join(', ') || '—'),
      h('dt', {}, 'Responsable'), h('dd', {}, i.commander || '—'), h('dt', {}, 'Résumé'), h('dd', {}, i.summary || '—'),
      h('dt', {}, 'Opérations liées'), h('dd', {}, i.linked_operations.length ? i.linked_operations.map((o) => h('a', { href: `#/operations/${o}`, style: 'margin-right:8px' }, short(o, 8))) : '—'))),
    card('Chronologie', h('ul', { class: 'timeline' }, i.timeline.map((t) => h('li', {}, h('div', { class: 'when' }, `${fmtDate(t.at)} · ${t.author}`), t.message)))));
}

// ------------------------------------------------------- sauvegardes, maintenance
async function viewBackups() {
  const list = await get('/backups');
  const run = async (b, kind) => {
    try {
      const op = await post('/operations', { environment: b.environment, action: { kind, policy: b.policy }, reason: 'Lancement manuel depuis la console' });
      location.hash = `#/operations/${op.id}`;
    } catch (e) { toast(e.message, true); }
  };
  const min = (s) => (s == null ? '—' : `${Math.round(s / 60)} min`);
  return h('div', {}, pageHead('Sauvegardes', 'Un seul modèle de politique (restic, pgBackRest, Velero, commandes fournisseur), RPO surveillé, exercices de restauration mesurés contre le RTO.'),
    card(null, tableOf(['Politique', 'Env.', 'Source', 'RPO', 'Dernière réussite', 'RPO', 'Exercice', 'RTO', 'Actions'], list.map((b) => [b.policy, [
      h('strong', {}, b.policy), b.environment, b.source, min(b.rpo_secs), ago(b.last_success),
      b.rpo_compliant ? h('span', { class: 'pill ok' }, 'tenu') : h('span', { class: 'pill bad' }, 'dépassé'),
      b.drill ? (b.drill.last_success ? `${ago(b.drill.last_success)} (${b.drill.last_duration_secs} s)` : h('span', { class: 'pill warn' }, 'jamais')) : '—',
      b.drill?.within_rto === false ? h('span', { class: 'pill bad' }, `> ${min(b.rto_secs)}`) : min(b.rto_secs),
      h('div', { class: 'actions' }, h('button', { class: 'btn small', onclick: () => run(b, 'backup') }, 'Sauvegarder'), b.drill ? h('button', { class: 'btn small', onclick: () => run(b, 'restore_drill') }, 'Exercice') : null)]]))));
}

async function viewMaintenance() {
  const list = await get('/maintenance');
  const env = h('select', {}, Object.keys(state.spec.environments).map((e) => h('option', { value: e }, e)));
  const duration = h('input', { value: '2h' });
  const reason = h('input', { placeholder: 'Ex. : migration de la base, montée de version du cluster' });
  const start = async (e) => {
    e.preventDefault();
    try { await post('/maintenance', { environment: env.value, duration: duration.value, reason: reason.value }); toast('Maintenance démarrée.'); refresh(); } catch (err) { toast(err.message, true); }
  };
  const end = async (id) => { try { await post(`/maintenance/${id}/end`); toast('Maintenance terminée.'); refresh(); } catch (err) { toast(err.message, true); } };
  return h('div', {}, pageHead('Maintenance', 'Gel des déploiements et silence des alertes pendant une intervention planifiée.'),
    card('Démarrer une maintenance', h('form', { onsubmit: start }, h('div', { class: 'row' },
      h('div', {}, h('label', {}, 'Environnement'), env), h('div', {}, h('label', {}, 'Durée'), duration), h('div', {}, h('label', {}, 'Motif'), reason)),
      h('button', { class: 'btn primary', type: 'submit' }, 'Démarrer'))),
    card('Fenêtres', tableOf(['Env.', 'Statut', 'Début', 'Fin', 'Motif', 'Par', ''], list.map((m) => [m.id, [
      m.environment, m.status === 'active' ? h('span', { class: 'pill warn' }, 'active') : h('span', { class: 'pill' }, 'terminée'),
      fmtDate(m.starts_at), fmtDate(m.ends_at), m.reason, m.created_by,
      m.status === 'active' ? h('button', { class: 'btn small', onclick: () => end(m.id) }, 'Terminer') : '']]))));
}

// ------------------------------------------------------ audit, conformité, jetons
async function viewAudit() {
  const events = await get('/audit?limit=200');
  const result = h('div', {});
  const verify = async () => {
    const r = await get('/audit/verify');
    result.replaceChildren(r.valid ? h('div', { class: 'banner', style: 'background:var(--ok-soft);color:var(--ok)' }, `Chaîne intègre — ${r.events_checked} événements vérifiés, tête ${short(r.head_hash)}.`)
      : h('div', { class: 'banner bad' }, `Chaîne ROMPUE à la séquence ${r.broken_at} : ${r.reason}`));
  };
  return h('div', {}, pageHead('Journal d’audit', 'Chaque événement est scellé par l’empreinte du précédent (SHA-256) : toute altération est détectée.',
    h('button', { class: 'btn', onclick: verify }, 'Vérifier l’intégrité'), h('button', { class: 'btn', onclick: () => download('/audit/export', 'socle-audit.jsonl') }, 'Exporter (JSONL)')),
  result,
  card(null, tableOf(['Séq.', 'Horodatage', 'Acteur', 'Action', 'Ressource', 'Résultat', 'Empreinte'], events.map((e) => [e.seq, [
    e.seq, fmtDate(e.ts), e.actor, e.action, h('span', { class: 'mono' }, e.resource), e.outcome, h('span', { class: 'mono', title: e.hash }, short(e.hash, 10))]]))));
}

async function viewCompliance() {
  const r = await get('/compliance?days=90');
  return h('div', {}, pageHead('Conformité', `ISO 27001 / SOC 2 — ${r.period_days} derniers jours, généré ${fmtDate(r.generated_at)}`,
    h('button', { class: 'btn', onclick: () => download('/compliance?days=90&format=md', 'socle-conformite.md') }, 'Dossier de preuves (Markdown)')),
  card('Contrôles', tableOf(['Référence', 'Contrôle', 'Statut', 'Preuve'], r.controls.map((c) => [c.reference, [
    h('span', { class: 'mono' }, c.reference), c.title, h('span', { class: `pill ${c.status === 'conforme' ? 'ok' : 'bad'}` }, c.status), c.evidence]]))),
  h('div', { class: 'grid kpis' }, [
    ['Changements', r.changes_total], ['En production', `${r.production_changes_approved}/${r.production_changes}`],
    ['Urgences', r.emergency_changes.length], ['Refus par les portes', r.gate_rejections],
    ['MTTA', r.mean_time_to_acknowledge_min == null ? '—' : `${r.mean_time_to_acknowledge_min.toFixed(0)} min`],
    ['MTTR', r.mean_time_to_resolve_min == null ? '—' : `${r.mean_time_to_resolve_min.toFixed(0)} min`],
  ].map(([l, v]) => h('div', { class: 'card kpi' }, h('div', { class: 'label' }, l), h('div', { class: 'value' }, v)))),
  r.emergency_changes.length ? card('Changements d’urgence (revue a posteriori)', h('ul', {}, r.emergency_changes.map((e) => h('li', {}, e)))) : null,
  r.postmortems_pending.length ? card('Postmortems en attente', h('ul', {}, r.postmortems_pending.map((e) => h('li', {}, e)))) : null);
}

async function viewTokens() {
  const list = await get('/tokens');
  const name = h('input', { placeholder: 'ci-github' });
  const subject = h('input', { placeholder: 'github:org/repo' });
  const teams = h('select', { multiple: true, size: 4 }, Object.keys(state.spec.teams).map((t) => h('option', { value: t }, t)));
  const ttl = h('input', { value: '90d' });
  const secret = h('div', {});
  const create = async (e) => {
    e.preventDefault();
    try {
      const r = await post('/tokens', { name: name.value, subject: subject.value, teams: [...teams.selectedOptions].map((o) => o.value), ttl: ttl.value || null });
      secret.replaceChildren(h('div', { class: 'banner warn' }, 'Copiez ce secret maintenant : il ne sera plus jamais affiché.'), h('div', { class: 'secret' }, r.secret));
    } catch (err) { toast(err.message, true); }
  };
  const revoke = async (id) => { try { await api('DELETE', `/tokens/${id}`); toast('Jeton révoqué.'); refresh(); } catch (err) { toast(err.message, true); } };
  return h('div', {}, pageHead('Jetons d’API', 'Pour la CI et les automatisations. Les droits viennent des équipes (section access de socle.yaml).'),
    card('Créer un jeton', h('form', { onsubmit: create }, h('div', { class: 'row' },
      h('div', {}, h('label', {}, 'Nom'), name), h('div', {}, h('label', {}, 'Sujet'), subject), h('div', {}, h('label', {}, 'Équipes'), teams), h('div', {}, h('label', {}, 'Durée de vie'), ttl)),
      h('button', { class: 'btn primary', type: 'submit' }, 'Créer')), secret),
    card('Jetons', tableOf(['Nom', 'Sujet', 'Équipes', 'Créé', 'Expiration', 'Dernier usage', 'État', ''], list.map((t) => [t.id, [
      t.name, t.subject, [...t.teams].join(', '), fmtDate(t.created_at), t.expires_at ? fmtDate(t.expires_at) : h('span', { class: 'pill warn' }, 'jamais'),
      t.last_used_at ? ago(t.last_used_at) : '—', t.revoked ? h('span', { class: 'pill' }, 'révoqué') : h('span', { class: 'pill ok' }, 'actif'),
      t.revoked ? '' : h('button', { class: 'btn small danger', onclick: () => revoke(t.id) }, 'Révoquer')]]))));
}

// --------------------------------------------------------------- démarrage
if (state.token) boot().catch((e) => renderLogin(e.message)); else renderLogin();
