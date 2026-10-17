// Diagram editor of Loonar Compass: shows an Enterprise Architect diagram with maxGraph, lets the user move, resize,
// align, rename, add elements from the toolbox and connect them, then sends the changes to the application, which
// saves them in EA through its own operations. The diagram comes in window.__DIAGRAM__ (set by the application).
import {
  Graph, InternalEvent, RubberBandHandler, getDefaultPlugins, UndoManager, Point,
} from '@maxgraph/core';

const data = window.__DIAGRAM__;
const t = data.texts;
const container = document.getElementById('graph');
const graph = new Graph(container, undefined, [...getDefaultPlugins(), RubberBandHandler]);
graph.setPanning(true);
graph.setConnectable(true);
graph.setAllowDanglingEdges(false);
graph.setDisconnectOnMove(false);
graph.setHtmlLabels(true);
graph.gridSize = 10;
graph.setGridEnabled?.(true);
graph.getPlugin('SelectionHandler').guidesEnabled = true;
graph.getPlugin('ConnectionHandler').createTarget = false;
InternalEvent.disableContextMenu(container);

// --- metadata of every cell: the EA object behind it, or a new one -------------------------------------------------

const meta = new Map(); // cell id -> {kind: 'element'|'connector', ea: {...}, isNew}
const original = new Map(); // cell id -> {name, x, y, w, h, points}

const BPMN = new Set(['Activity', 'StartEvent', 'EndEvent', 'IntermediateEvent', 'Gateway', 'Pool', 'Lane', 'SequenceFlow', 'MessageFlow']);
const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

function vertexStyle(el) {
  const st = el.stereotype || '';
  if (el.type === 'Event' || st.endsWith('Event')) {
    const end = st.includes('End');
    return { shape: 'ellipse', perimeter: 'ellipsePerimeter', fillColor: end ? '#FDECEA' : '#EAF7EE', strokeColor: end ? '#B3261E' : '#1D7A46', strokeWidth: end ? 3 : 1.5, verticalLabelPosition: 'bottom', verticalAlign: 'top', fontColor: '#2A3A47', fontSize: 11, resizable: false };
  }
  if (el.type === 'Decision' || st.includes('Gateway')) {
    return { shape: 'rhombus', perimeter: 'rhombusPerimeter', fillColor: '#FFF8D6', strokeColor: '#B8A26A', verticalLabelPosition: 'bottom', verticalAlign: 'top', fontSize: 11 };
  }
  if (el.type === 'Note' || el.type === 'Text') {
    return { fillColor: '#FFFDE7', strokeColor: '#C9B458', align: 'left', verticalAlign: 'top', spacing: 6, whiteSpace: 'wrap', fontSize: 11 };
  }
  if (el.type === 'Activity' || el.type === 'Action') {
    const container = el.contains;
    return { rounded: true, arcSize: 12, absoluteArcSize: true, fillColor: container ? '#FFFBEA' : '#FFF8D6', strokeColor: '#B8A26A', whiteSpace: 'wrap', fontSize: 12, verticalAlign: container ? 'top' : 'middle', spacingTop: container ? 6 : 0, fontStyle: container ? 1 : 0 };
  }
  if (['Class', 'Interface', 'Component', 'Enumeration', 'DataType', 'Signal'].includes(el.type)) {
    return { fillColor: '#FFFFFF', strokeColor: '#0E4A6B', verticalAlign: 'top', align: 'left', spacing: 0, overflow: 'fill', whiteSpace: 'wrap', fontSize: 12 };
  }
  if (el.type === 'UseCase') return { shape: 'ellipse', perimeter: 'ellipsePerimeter', fillColor: '#FFF8D6', strokeColor: '#B8A26A', whiteSpace: 'wrap' };
  if (el.type === 'Actor') return { shape: 'actor', fillColor: '#FFFFFF', strokeColor: '#0E4A6B', verticalLabelPosition: 'bottom', verticalAlign: 'top' };
  if (el.type === 'Package') return { fillColor: '#F1F5F9', strokeColor: '#55667A', verticalAlign: 'top', fontStyle: 1 };
  return { fillColor: '#FFFFFF', strokeColor: '#55667A', whiteSpace: 'wrap' };
}

function edgeStyle(c) {
  const s = { edgeStyle: 'orthogonalEdgeStyle', rounded: true, strokeColor: '#3B4B59', fontSize: 11, labelBackgroundColor: '#FFFFFF', endArrow: 'none' };
  const st = c.stereotype || '';
  switch (c.type) {
    case 'Generalization': return { ...s, endArrow: 'block', endFill: false, endSize: 12 };
    case 'Realisation': case 'Realization': return { ...s, endArrow: 'block', endFill: false, endSize: 12, dashed: true };
    case 'Aggregation': return { ...s, endArrow: 'diamond', endFill: c.subtype === 'Strong', endSize: 14 };
    case 'Dependency': case 'Usage': return { ...s, endArrow: 'open', dashed: true };
    case 'ControlFlow': case 'ObjectFlow': case 'StateFlow': return { ...s, endArrow: 'block', endFill: true };
    default: return st === 'SequenceFlow' ? { ...s, endArrow: 'block', endFill: true } : s;
  }
}

// Classes show their attributes and operations in compartments; the label text itself is the name only.
function classLabel(cell, ea) {
  const rows = (items) => items.map((x) => `<div class="row">${esc(x)}</div>`).join('');
  const stereo = ea.type === 'Interface' ? '<div class="st">«interface»</div>' : (ea.stereotype && !BPMN.has(ea.stereotype) ? `<div class="st">«${esc(ea.stereotype)}»</div>` : '');
  return `<div class="cls">${stereo}<div class="nm">${esc(cell.value)}</div><div class="cmp">${rows(ea.attributes || [])}</div><div class="cmp">${rows(ea.operations || [])}</div></div>`;
}
const baseGetLabel = graph.getLabel.bind(graph);
graph.getLabel = (cell) => {
  const m = meta.get(cell.id);
  if (m && m.kind === 'element' && ['Class', 'Interface', 'Enumeration', 'DataType'].includes(m.ea.type)) return classLabel(cell, m.ea);
  return m && cell.isVertex() ? esc(cell.value) : baseGetLabel(cell);
};
// Editing a label edits the name only (plain text, not the class compartments).
graph.isHtmlLabel = () => true;
graph.getEditingValue = (cell) => String(cell.value ?? '');

// --- building the diagram ------------------------------------------------------------------------------------------

const cells = new Map(); // EA object id -> vertex
const parent = graph.getDefaultParent();
graph.batchUpdate(() => {
  const ids = new Set(data.elements.map((e) => e.id));
  for (const el of data.elements) el.contains = data.elements.some((o) => o.parent === el.id);
  // Larger elements first, so frames and subprocesses stay behind their contents.
  const order = [...data.elements].sort((a, b) => b.width * b.height - a.width * a.height);
  for (const el of order) {
    const v = graph.insertVertex({ parent, id: `e${el.id}`, value: el.name, position: [el.left, el.top], size: [el.width, el.height], style: vertexStyle(el) });
    cells.set(el.id, v);
    meta.set(v.id, { kind: 'element', ea: el, isNew: false });
    original.set(v.id, { name: el.name, x: el.left, y: el.top, w: el.width, h: el.height });
  }
  for (const c of data.connectors) {
    if (!ids.has(c.source) || !ids.has(c.target)) continue;
    const e = graph.insertEdge({ parent, id: `c${c.id}`, value: c.name || '', source: cells.get(c.source), target: cells.get(c.target), style: edgeStyle(c) });
    if (c.points && c.points.length) e.geometry.points = c.points.map(([x, y]) => new Point(x, y));
    meta.set(e.id, { kind: 'connector', ea: c, isNew: false });
    original.set(e.id, { name: c.name || '', points: JSON.stringify(c.points || []) });
  }
});

// --- undo, delete of new cells ---------------------------------------------------------------------------------

const undo = new UndoManager();
const remember = (_s, evt) => undo.undoableEditHappened(evt.getProperty('edit'));
graph.getDataModel().addListener(InternalEvent.UNDO, remember);
graph.getView().addListener(InternalEvent.UNDO, remember);
document.addEventListener('keydown', (ev) => {
  if (graph.isEditing()) return;
  if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === 'z') { undo.undo(); ev.preventDefault(); }
  if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === 'y') { undo.redo(); ev.preventDefault(); }
  if (ev.key === 'Delete' || ev.key === 'Backspace') {
    const selected = graph.getSelectionCells().filter((c) => meta.get(c.id));
    if (!selected.length) return;
    // What was added in this editor is simply removed; existing model objects ask: from this diagram or from the model.
    const added = selected.filter((c) => meta.get(c.id).isNew);
    const existing = selected.filter((c) => !meta.get(c.id).isNew);
    if (added.length) graph.removeCells(added, true);
    if (existing.length) askDelete(existing);
    ev.preventDefault();
  }
});

// --- deleting existing objects: from this diagram or from the model ------------------------------------------------

const choices = new Map(); // cell id -> 'diagram' | 'model' (cells the user deleted explicitly)
function askDelete(list) {
  const overlay = document.getElementById('delete');
  const names = list.map((c) => meta.get(c.id).ea.name || String(c.value || '') || '—');
  const elements = list.filter((c) => c.isVertex());
  const others = [...new Set(list.flatMap((c) => meta.get(c.id).ea.other_diagrams || []))];
  const relations = elements.reduce((n, c) => n + (meta.get(c.id).ea.relations || 0), 0);
  const onlyConnectors = elements.length === 0;
  overlay.querySelector('.d-names').textContent = names.slice(0, 6).join(', ') + (names.length > 6 ? ` (+${names.length - 6})` : '');
  overlay.querySelector('[data-d="diagram"] b').textContent = onlyConnectors ? t.hide_connector : t.delete_from_diagram;
  overlay.querySelector('[data-d="diagram"] span').textContent = onlyConnectors ? t.hide_connector_hint : t.delete_from_diagram_hint;
  overlay.querySelector('[data-d="model"] b').textContent = t.delete_from_model;
  const usage = [others.length ? t.used_on.replace('{0}', others.length).replace('{1}', others.slice(0, 8).join(', ') + (others.length > 8 ? '…' : '')) : t.used_nowhere];
  if (elements.length) usage.push(t.relations.replace('{0}', relations));
  overlay.querySelector('[data-d="model"] span').textContent = `${t.delete_from_model_hint} ${usage.join(' ')}`;
  overlay.hidden = false;
  overlay.onclick = (e) => {
    const choice = e.target.closest('[data-d]')?.dataset.d;
    if (!choice) return;
    overlay.hidden = true;
    if (choice === 'cancel') return;
    list.forEach((c) => choices.set(c.id, choice));
    graph.removeCells(list, true);
  };
}

// --- toolbox and new connectors ----------------------------------------------------------------------------------

let counter = 0;
const toolbox = document.getElementById('toolbox');
for (const tool of data.toolbox) {
  const b = document.createElement('button');
  b.textContent = tool.label;
  b.title = t.add_hint;
  b.addEventListener('click', () => {
    const s = graph.getView().scale;
    const tr = graph.getView().translate;
    const x = Math.round((container.scrollLeft + container.clientWidth / 2) / s - tr.x - tool.width / 2);
    const y = Math.round((container.scrollTop + container.clientHeight / 2) / s - tr.y - tool.height / 2);
    const ea = { type: tool.type, stereotype: tool.stereotype || '', attributes: [], operations: [] };
    let v;
    graph.batchUpdate(() => {
      v = graph.insertVertex({ parent, id: `n${++counter}`, value: tool.name, position: [x + counter * 10, y + counter * 10], size: [tool.width, tool.height], style: vertexStyle(ea) });
      meta.set(v.id, { kind: 'element', ea, isNew: true });
    });
    graph.setSelectionCell(v);
    graph.startEditingAtCell(v);
  });
  toolbox.appendChild(b);
}
const connType = document.getElementById('conntype');
data.connector_types.forEach((ct, i) => {
  const o = document.createElement('option');
  o.value = String(i);
  o.textContent = ct.label;
  connType.appendChild(o);
});
graph.getPlugin('ConnectionHandler').addListener(InternalEvent.CONNECT, (_s, evt) => {
  const edge = evt.getProperty('cell');
  const ct = data.connector_types[Number(connType.value) || 0];
  const ea = { type: ct.type, stereotype: ct.stereotype || '', name: '' };
  meta.set(edge.id, { kind: 'connector', ea, isNew: true });
  graph.setCellStyle(edgeStyle(ea), [edge]);
});

// --- alignment and zoom ---------------------------------------------------------------------------------------------

const vertices = () => graph.getSelectionCells().filter((c) => c.isVertex());
const actions = {
  'align-left': () => graph.alignCells('left', vertices()),
  'align-center': () => graph.alignCells('center', vertices()),
  'align-top': () => graph.alignCells('top', vertices()),
  'align-middle': () => graph.alignCells('middle', vertices()),
  'same-size': () => {
    const sel = vertices();
    if (sel.length < 2) return;
    const g0 = sel[0].geometry;
    graph.batchUpdate(() => sel.slice(1).forEach((c) => { const g = c.geometry.clone(); g.width = g0.width; g.height = g0.height; graph.getDataModel().setGeometry(c, g); }));
  },
  'zoom-in': () => graph.zoomIn(),
  'zoom-out': () => graph.zoomOut(),
  'zoom-fit': () => graph.getPlugin('fit')?.fitCenter({ margin: 20 }),
};
document.querySelectorAll('[data-action]').forEach((b) => b.addEventListener('click', () => actions[b.dataset.action]()));

// --- saving: only what changed goes back to the application -----------------------------------------------------------

const round = (n) => Math.round(n);
function changes() {
  const out = { action: 'save', moved: [], renamed: [], routed: [], created: [], connected: [], removed: [], hidden: [], deleted: [], deleted_connectors: [] };
  // Existing objects that are no longer on the diagram: what the user chose for them (elements removed with them
  // take their connectors off this diagram without any separate operation).
  const present = graph.getDataModel().cells;
  for (const [id, m] of meta) {
    if (m.isNew || present[id]) continue;
    const choice = choices.get(id);
    if (!choice) continue;
    if (m.kind === 'element') (choice === 'model' ? out.deleted : out.removed).push(m.ea.id);
    else (choice === 'model' ? out.deleted_connectors : out.hidden).push(m.ea.id);
  }
  const key = (cell) => (meta.get(cell.id)?.isNew ? `#${cell.id}` : meta.get(cell.id).ea.id);
  for (const cell of Object.values(graph.getDataModel().cells)) {
    const m = meta.get(cell.id);
    if (!m) continue;
    if (cell.isVertex()) {
      const g = cell.geometry;
      const box = { left: round(g.x), top: round(g.y), width: round(g.width), height: round(g.height) };
      if (m.isNew) {
        out.created.push({ key: cell.id, name: String(cell.value || ''), type: m.ea.type, stereotype: m.ea.stereotype, ...box });
        continue;
      }
      const o = original.get(cell.id);
      if (box.left !== o.x || box.top !== o.y || box.width !== o.w || box.height !== o.h) out.moved.push({ id: m.ea.id, ...box });
      if (String(cell.value) !== o.name) out.renamed.push({ id: m.ea.id, name: String(cell.value) });
    } else if (cell.isEdge()) {
      if (!cell.source || !cell.target) continue;
      const points = (cell.geometry.points || []).map((p) => [round(p.x), round(p.y)]);
      if (m.isNew) {
        out.connected.push({ key: cell.id, type: m.ea.type, stereotype: m.ea.stereotype, name: String(cell.value || ''), source: key(cell.source), target: key(cell.target), points });
        continue;
      }
      const o = original.get(cell.id);
      if (JSON.stringify(points) !== o.points) out.routed.push({ id: m.ea.id, points });
      if (String(cell.value || '') !== o.name) out.renamed_connectors = [...(out.renamed_connectors || []), { id: m.ea.id, name: String(cell.value || '') }];
    }
  }
  return out;
}
function send(message) {
  window.ipc.postMessage(JSON.stringify(message));
}
document.getElementById('save').addEventListener('click', () => {
  const c = changes();
  const count = c.moved.length + c.renamed.length + c.routed.length + c.created.length + c.connected.length
    + c.removed.length + c.hidden.length + c.deleted.length + c.deleted_connectors.length;
  if (!count) { document.getElementById('status').textContent = t.nothing; return; }
  send(c);
});
document.getElementById('cancel').addEventListener('click', () => send({ action: 'cancel' }));
document.getElementById('title').textContent = data.diagram.name;
graph.getPlugin('fit')?.fitCenter({ margin: 20 });
if (graph.getView().scale > 1) graph.zoomActual();

// Development self-test (LOONAR_EDITOR_SELFTEST, debug builds only): scripted edits, then saving.
if (data.selftest) {
  setTimeout(() => {
    const byName = (n) => Object.values(graph.getDataModel().cells).find((c) => c.isVertex() && c.value === n);
    const cust = byName('Customer');
    const vip = byName('VipCustomer');
    graph.batchUpdate(() => {
      const g = vip.geometry.clone();
      g.x = cust.geometry.x; g.y = cust.geometry.y + cust.geometry.height + 80;
      graph.getDataModel().setGeometry(vip, g);
      graph.getDataModel().setValue(byName('OrderLine'), 'OrderItem');
    });
    document.querySelector('#toolbox button').click();
    const added = graph.getSelectionCell();
    graph.stopEditing(false);
    graph.getDataModel().setValue(added, 'Invoice');
    const e = graph.insertEdge({ parent, id: 'n99', value: 'billed by', source: byName('Order'), target: added, style: edgeStyle({ type: 'Association' }) });
    meta.set(e.id, { kind: 'connector', ea: { type: 'Association', stereotype: '' }, isNew: true });
    setTimeout(() => document.getElementById('save').click(), 1500);
  }, 1500);
}
