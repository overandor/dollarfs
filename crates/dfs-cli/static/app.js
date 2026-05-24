const api = async (path) => {
  const r = await fetch(path);
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  return r.json();
};

function el(id) { return document.getElementById(id); }

function fmtTime(ts) {
  if (!ts) return '--';
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString() + ' ' + d.toLocaleDateString();
}

function fmtIso(iso) {
  if (!iso) return '--';
  const d = new Date(iso);
  return d.toLocaleTimeString() + ' ' + d.toLocaleDateString();
}

function shortHash(h, n=8) { return h ? h.slice(0, n) : '--'; }

function fmtUsd(v) {
  if (v === undefined || v === null) return '-';
  return '$' + v.toFixed(2);
}

let directories = [];

async function loadStatus() {
  const data = await api('/api/status');
  el('stat-files').textContent = data.files ?? 0;
  el('stat-value').textContent = fmtUsd(data.total_value);
  el('stat-security').textContent = data.security_findings ?? 0;
  el('stat-ledger').textContent = data.ledger?.block_height ?? 0;
}

async function loadTop() {
  const data = await api('/api/top?limit=20');
  const top = data.top || [];
  let html = '<table><thead><tr><th>Path</th><th>Value</th><th>Confidence</th><th>Reason</th></tr></thead><tbody>';
  for (const t of top) {
    const conf = t.confidence ?? 0;
    const confLabel = conf >= 0.8 ? 'High' : (conf >= 0.5 ? 'Med' : 'Low');
    const confClass = conf >= 0.8 ? 'val-cyan' : (conf >= 0.5 ? '' : 'val-red');
    const reason = (t.reason || '').substring(0, 50);
    html += `<tr><td>${t.path}</td><td class="val-cyan">${fmtUsd(t.value)}</td><td class="${confClass}">${confLabel}</td><td>${reason}</td></tr>`;
  }
  html += '</tbody></table>';
  if (top.length === 0) html = '<p style="opacity:0.7;font-size:0.8rem">No valuations yet.</p>';
  el('top-table').innerHTML = html;
}

async function loadSecurity() {
  const data = await api('/api/security?limit=50');
  const findings = data.findings || [];
  let html = '<table><thead><tr><th>Path</th><th>Line</th><th>Type</th><th>Severity</th><th>Preview</th></tr></thead><tbody>';
  for (const f of findings) {
    const badgeClass = f.severity === 'critical' ? 'critical' : (f.severity === 'high' ? 'high' : 'file');
    html += `<tr><td>${f.path}</td><td>${f.line}</td><td>${f.finding_type}</td><td><span class="badge ${badgeClass}">${f.severity}</span></td><td>${f.preview}</td></tr>`;
  }
  html += '</tbody></table>';
  if (findings.length === 0) html = '<p style="opacity:0.7;font-size:0.8rem">No security findings.</p>';
  el('security-table').innerHTML = html;
}

async function loadLedger() {
  const data = await api('/api/ledger?limit=50');
  const blocks = data.blocks || [];
  let html = '<table><thead><tr><th>Index</th><th>Timestamp</th><th>Hash</th><th>Type</th><th>Data</th></tr></thead><tbody>';
  for (const b of blocks.slice().reverse()) {
    const d = b.data || {};
    const type = d.type || '-';
    const badge = `<span class="badge ${type === 'file_flagged_secret' ? 'error' : (type.startsWith('agent') ? 'agent' : 'file')}">${type}</span>`;
    const preview = d.notes ? d.notes.substring(0, 60) : (d.path || '-');
    html += `<tr><td>#${b.index}</td><td>${fmtIso(b.timestamp)}</td><td class="hash">${shortHash(b.hash, 12)}</td><td>${badge}</td><td>${preview}</td></tr>`;
  }
  html += '</tbody></table>';
  if (blocks.length === 0) html = '<p style="opacity:0.7;font-size:0.8rem">Ledger is empty.</p>';
  el('ledger-table').innerHTML = html;
}

async function loadEvents() {
  const data = await api('/api/events?limit=50');
  const ev = data.events || [];
  let html = '<table><thead><tr><th>Time</th><th>Type</th><th>Path</th><th>Source</th></tr></thead><tbody>';
  for (const e of ev) {
    const type = e.type || 'unknown';
    const badge = `<span class="badge ${type.startsWith('agent') ? 'agent' : 'file'}">${type}</span>`;
    html += `<tr><td>${fmtTime(e.timestamp)}</td><td>${badge}</td><td>${e.path}</td><td>${e.source || '-'}</td></tr>`;
  }
  html += '</tbody></table>';
  if (ev.length === 0) html = '<p style="opacity:0.7;font-size:0.8rem">No events recorded yet.</p>';
  el('events-table').innerHTML = html;
}

async function loadFiles() {
  const data = await api('/api/files?limit=30');
  const files = data.files || [];
  let html = '<table><thead><tr><th>Path</th><th>Size</th><th>Value</th><th>Confidence</th></tr></thead><tbody>';
  for (const f of files) {
    const val = f.value !== null ? fmtUsd(f.value) : '-';
    html += `<tr><td>${f.path}</td><td>${f.size}</td><td class="val-cyan">${val}</td><td>${f.confidence !== null ? (f.confidence * 100).toFixed(0) + '%' : '-'}</td></tr>`;
  }
  html += '</tbody></table>';
  if (files.length === 0) html = '<p style="opacity:0.7;font-size:0.8rem">No files indexed.</p>';
  el('files-table').innerHTML = html;
}

async function loadCommits() {
  const sel = el('repo-select');
  const repo = sel.value;
  if (!repo) {
    el('commits-table').innerHTML = 'Select a directory above.';
    return;
  }
  const data = await api('/api/commits?dir=' + encodeURIComponent(repo));
  const commits = data.commits || [];
  let html = '<table><thead><tr><th>Hash</th><th>Time</th><th>Message</th></tr></thead><tbody>';
  for (const c of commits) {
    html += `<tr><td class="hash">${c.hash}</td><td>${fmtIso(c.timestamp)}</td><td>${c.message}</td></tr>`;
  }
  html += '</tbody></table>';
  if (commits.length === 0) html = '<p style="opacity:0.7;font-size:0.8rem">No commits found.</p>';
  el('commits-table').innerHTML = html;
}

function updateClock() {
  const now = new Date();
  el('clock').textContent = now.toLocaleTimeString();
}

async function refreshAll() {
  try { await loadStatus(); } catch (e) { console.error(e); }
  try { await loadTop(); } catch (e) { console.error(e); }
  try { await loadSecurity(); } catch (e) { console.error(e); }
  try { await loadLedger(); } catch (e) { console.error(e); }
  try { await loadEvents(); } catch (e) { console.error(e); }
  try { await loadFiles(); } catch (e) { console.error(e); }
  try { await loadCommits(); } catch (e) { console.error(e); }
}

// Populate repo selector from file paths
topLevelDirs = [];
async function loadTopDirs() {
  const data = await api('/api/files?limit=200');
  const files = data.files || [];
  const dirs = new Set();
  for (const f of files) {
    const parts = f.path.split('/');
    if (parts.length >= 3) {
      dirs.add(parts.slice(0, 3).join('/'));
    }
  }
  const sel = el('repo-select');
  const current = sel.value;
  sel.innerHTML = '<option value="">Select directory...</option>';
  for (const d of dirs) {
    const opt = document.createElement('option');
    opt.value = d;
    opt.textContent = d;
    sel.appendChild(opt);
  }
  if (current) sel.value = current;
}

el('repo-select').addEventListener('change', loadCommits);

setInterval(updateClock, 1000);
updateClock();
refreshAll();
loadTopDirs();
setInterval(refreshAll, 5000);
