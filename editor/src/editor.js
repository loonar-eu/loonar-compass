// Diagram editor of Loonar Compass: shows an Enterprise Architect diagram with maxGraph, lets the user move, resize,
// align, rename, add elements from the toolbox and connect them, then sends the changes to the application, which
// saves them in EA through its own operations. The diagram comes in window.__DIAGRAM__ (set by the application).
import {
  Graph, InternalEvent, RubberBandHandler, getDefaultPlugins, UndoManager, Point, ImageBox, CellOverlay,
} from '@maxgraph/core';

const data = window.__DIAGRAM__;
const t = data.texts;
const SEQ = data.diagram.type === 'Sequence';
// On a sequence diagram: the participants (a header with a dashed line down), not fragments, notes or the frame.
const isLifeline = (ea) => SEQ && !!ea && !['InteractionFragment', 'Note', 'Text', 'Boundary'].includes(ea.type);
const HEADER = 50;
const STEP = 40;
const seqOrder = []; // message edges of a sequence diagram, from top to bottom
const isMessage = (cell) => SEQ && !!cell && cell.isEdge() && meta.get(cell.id)?.ea.type === 'Sequence';
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
const edits = new Map(); // cell id -> {fields: {...}, tags: {name: value|null}, bgcolor?, line_style?}

const BPMN = new Set(['Activity', 'StartEvent', 'EndEvent', 'IntermediateEvent', 'Gateway', 'Pool', 'Lane', 'SequenceFlow', 'MessageFlow']);
// Notes show their text (EA keeps it in the notes, which may hold simple HTML formatting).
const isNote = (ea) => ea && (ea.type === 'Note' || ea.type === 'Text');
const plainNotes = (s) => String(s || '').replace(/<br\s*\/?>/gi, '\n').replace(/<[^>]+>/g, '').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&');
const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

function vertexStyle(el) {
  const st = el.stereotype || '';
  if (isLifeline(el)) return { fillColor: '#FFFFFF', fillOpacity: 0, strokeColor: 'none', verticalAlign: 'top', overflow: 'fill', whiteSpace: 'wrap', fontSize: 12 };
  if (el.type === 'StateNode') {
    // Pseudostates: initial (filled), final (ring), choice (diamond).
    if ([3, 100].includes(el.subtype)) return { shape: 'ellipse', perimeter: 'ellipsePerimeter', fillColor: '#1F2A33', strokeColor: '#1F2A33', verticalLabelPosition: 'bottom', verticalAlign: 'top', fontSize: 11, resizable: false };
    if ([4, 101].includes(el.subtype)) return { shape: 'doubleEllipse', perimeter: 'ellipsePerimeter', fillColor: '#FFFFFF', strokeColor: '#1F2A33', strokeWidth: 2, verticalLabelPosition: 'bottom', verticalAlign: 'top', fontSize: 11, resizable: false };
    if (el.subtype === 11) return { shape: 'rhombus', perimeter: 'rhombusPerimeter', fillColor: '#FFFFFF', strokeColor: '#1F2A33', verticalLabelPosition: 'bottom', verticalAlign: 'top', fontSize: 11 };
    return { shape: 'ellipse', perimeter: 'ellipsePerimeter', fillColor: '#FFFFFF', strokeColor: '#1F2A33' };
  }
  if (el.type === 'Synchronization') return { fillColor: '#1F2A33', strokeColor: '#1F2A33', noLabel: true };
  if (el.type === 'State') return { rounded: true, arcSize: 30, fillColor: '#FFF8D6', strokeColor: '#B8A26A', whiteSpace: 'wrap', fontSize: 12 };
  if (['Node', 'Device', 'ExecutionEnvironment'].includes(el.type)) {
    return { fillColor: el.type === 'Device' ? '#EEF3F7' : '#F4F7FA', strokeColor: '#0E4A6B', strokeWidth: 2, verticalAlign: 'top', spacingTop: 6, fontStyle: 1, whiteSpace: 'wrap', shadow: true };
  }
  if (el.type === 'Artifact') return { fillColor: '#FFFFFF', strokeColor: '#55667A', whiteSpace: 'wrap', fontSize: 12 };
  if (el.type === 'InteractionFragment') return { fillColor: 'none', strokeColor: '#55667A', align: 'left', verticalAlign: 'top', spacingLeft: 6, spacingTop: 4, fontSize: 11 };
  if (st.endsWith('IntermediateEvent')) {
    return { shape: 'doubleEllipse', perimeter: 'ellipsePerimeter', fillColor: '#FFF8E6', strokeColor: '#9A6B00', strokeWidth: 1.5, verticalLabelPosition: 'bottom', verticalAlign: 'top', fontColor: '#2A3A47', fontSize: 11, resizable: false };
  }
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
    case 'ControlFlow': case 'ObjectFlow': return { ...s, endArrow: 'block', endFill: true };
    case 'StateFlow': return { ...s, endArrow: 'open', endFill: false };
    case 'Package': return { ...s, endArrow: 'open', dashed: true };
    default: return st === 'SequenceFlow' ? { ...s, endArrow: 'block', endFill: true } : s;
  }
}

// Classes show their attributes and operations in compartments; the label text itself is the name only.
function classLabel(cell, ea) {
  const rows = (items) => items.map((x) => `<div class="row">${esc(x)}</div>`).join('');
  const stereo = ea.type === 'Interface' ? '<div class="st">«interface»</div>' : (ea.stereotype && !BPMN.has(ea.stereotype) ? `<div class="st">«${esc(ea.stereotype)}»</div>` : '');
  return `<div class="cls">${stereo}<div class="nm"${current(cell, 'abstract') ? ' style="font-style:italic"' : ''}>${esc(cell.value)}</div><div class="cmp">${rows(ea.attributes || [])}</div><div class="cmp">${rows(ea.operations || [])}</div></div>`;
}
const baseGetLabel = graph.getLabel.bind(graph);
graph.getLabel = (cell) => {
  const m = meta.get(cell.id);
  if (m && cell.isVertex() && isLifeline(m.ea)) return `<div class="ll${m.ea.type === 'Actor' ? ' actor' : ''}"><div class="hd">${esc(cell.value)}</div><div class="ln"></div></div>`;
  if (m && isMessage(cell)) return `${seqOrder.indexOf(cell) + 1}: ${esc(cell.value)}`;
  const keyword = { Device: '«device»', ExecutionEnvironment: '«executionEnvironment»', Artifact: '«artifact»' }[m?.ea.type];
  if (m && cell.isVertex() && keyword) return `<div style="text-align:center"><div style="font-size:10px;font-weight:400;color:#55667A">${keyword}</div>${esc(cell.value)}</div>`;
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
    if (isNote(el)) el.name = plainNotes((el.props || {}).notes);
    const style = vertexStyle(el);
    const fill = (el.props || {}).bgcolor || (el.props || {}).color;
    if (fill) style.fillColor = fill;
    const v = graph.insertVertex({ parent, id: `e${el.id}`, value: el.name, position: [el.left, el.top], size: [el.width, el.height], style });
    cells.set(el.id, v);
    meta.set(v.id, { kind: 'element', ea: el, isNew: false });
    original.set(v.id, { name: el.name, x: el.left, y: el.top, w: el.width, h: el.height });
  }
  for (const c of data.connectors) {
    if (!ids.has(c.source) || !ids.has(c.target)) continue;
    const e = graph.insertEdge({ parent, id: `c${c.id}`, value: c.name || '', source: cells.get(c.source), target: cells.get(c.target), style: SEQ && c.type === 'Sequence' ? {} : edgeStyle(c) });
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
  // Typing in the properties panel is not an editor command.
  if (graph.isEditing() || ev.target.closest?.('input, textarea, select')) return;
  if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === 'z') { undo.undo(); ev.preventDefault(); }
  if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === 'y') { undo.redo(); ev.preventDefault(); }
  if (SEQ && ev.altKey && (ev.key === 'ArrowUp' || ev.key === 'ArrowDown')) {
    const cell = graph.getSelectionCell();
    if (isMessage(cell)) { moveMessage(cell, ev.key === 'ArrowUp' ? -1 : 1); ev.preventDefault(); }
  }
  if (ev.key === 'Delete' || ev.key === 'Backspace') {
    const selected = graph.getSelectionCells().filter((c) => meta.get(c.id));
    if (!selected.length) return;
    // What was added in this editor is simply removed; existing model objects ask: from this diagram or from the model.
    const fresh = (c) => meta.get(c.id).isNew || meta.get(c.id).placed;
    const added = selected.filter(fresh);
    const existing = selected.filter((c) => !fresh(c));
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
  overlay.querySelector('[data-d="diagram"]').hidden = list.every((c) => isMessage(c));
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

// --- properties panel: General, tagged values, appearance ---------------------------------------------------------
// Every field says what it changes: the model (everywhere the object is shown) or only this diagram.

const editOf = (id) => { if (!edits.has(id)) edits.set(id, { fields: {}, tags: {} }); return edits.get(id); };
const panel = document.getElementById('props');
let tab = 'general';

const EL_FIELDS = [
  ['alias', 'f_alias'], ['stereotype', 'f_stereotype'], ['keywords', 'f_keywords'], ['status', 'f_status', 'status'],
  ['version', 'f_version'], ['author', 'f_author'], ['phase', 'f_phase'], ['complexity', 'f_complexity', 'complexity'],
  ['abstract', 'f_abstract', 'bool'], ['notes', 'f_notes', 'text'],
];
const CONN_FIELDS = [
  ['stereotype', 'f_stereotype'], ['direction', 'f_direction', 'direction'],
  ['source_multiplicity', 'f_source_multiplicity', 'multiplicity'], ['source_role', 'f_source_role'],
  ['target_multiplicity', 'f_target_multiplicity', 'multiplicity'], ['target_role', 'f_target_role'],
  ['notes', 'f_notes', 'text'],
];
const DIRECTIONS = [['Unspecified', 'dir_none'], ['Source -> Destination', 'dir_forward'], ['Destination -> Source', 'dir_back'], ['Bi-Directional', 'dir_both']];
const LINE_STYLES = ['direct', 'auto', 'custom', 'orthogonal_square', 'orthogonal_rounded', 'tree_vertical', 'tree_horizontal', 'lateral_vertical', 'lateral_horizontal'];
const MULTIPLICITIES = ['', '0..1', '1', '0..*', '1..*', '*'];

const el = (tag, attrs = {}, ...kids) => {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'class') n.className = v; else if (k.startsWith('on')) n.addEventListener(k.slice(2), v); else if (v !== undefined) n.setAttribute(k, v);
  }
  for (const k of kids.flat()) if (k !== null && k !== undefined) n.append(k);
  return n;
};
const badge = (scope) => el('span', { class: `scope ${scope}`, title: scope === 'model' ? t.scope_model_hint : t.scope_diagram_hint }, scope === 'model' ? t.scope_model : t.scope_diagram);

// Current value of a field: what the user typed, or what the model has.
function current(cell, field) {
  const m = meta.get(cell.id);
  const e = edits.get(cell.id);
  if (e && field in e.fields) return e.fields[field];
  if (field === 'stereotype') return m.ea.stereotype || '';
  const v = (m.ea.props || {})[field];
  return field === 'abstract' ? !!v : (v ?? '');
}
function setField(cell, field, value) {
  const m = meta.get(cell.id);
  const original = field === 'stereotype' ? (m.ea.stereotype || '') : ((m.ea.props || {})[field] ?? (field === 'abstract' ? false : ''));
  const e = editOf(cell.id);
  if (value === original) delete e.fields[field]; else e.fields[field] = value;
}

function input(cell, field, kind) {
  const value = current(cell, field);
  const on = (v) => { setField(cell, field, v); if (isMessage(cell)) layoutMessages(); if (field === 'abstract') graph.refresh(cell); };
  if (kind === 'text') return el('textarea', { rows: '4', oninput: (ev) => on(ev.target.value) }, String(value));
  if (kind === 'bool') { const c = el('input', { type: 'checkbox', onchange: (ev) => on(ev.target.checked) }); c.checked = !!value; return c; }
  const select = (options) => {
    const s = el('select', { onchange: (ev) => on(ev.target.value) });
    const values = options.map((o) => o[0]);
    if (!values.includes(value)) options = [[value, value || '—'], ...options];
    for (const [v, label] of options) { const o = el('option', { value: v }, label); if (v === value) o.selected = true; s.append(o); }
    return s;
  };
  if (kind === 'status') return select((data.statuses.length ? data.statuses : ['Proposed', 'Approved', 'Implemented', 'Mandatory', 'Validated']).map((s) => [s, s]));
  if (kind === 'complexity') return select([['1', t.easy], ['2', t.medium], ['3', t.difficult]]);
  if (kind === 'direction') return select(DIRECTIONS.map(([v, k]) => [v, t[k]]));
  if (kind === 'sort') return select([['Synchronous', t.synchronous], ['Asynchronous', t.asynchronous]]);
  const i = el('input', { type: 'text', value: String(value), oninput: (ev) => on(ev.target.value) });
  if (kind === 'multiplicity') {
    i.setAttribute('list', 'multiplicities');
  }
  return i;
}
const row = (label, scope, control) => el('label', { class: 'f' }, el('span', { class: 'fl' }, label, scope ? badge(scope) : null), control);
const readonly = (label, value) => el('div', { class: 'f ro' }, el('span', { class: 'fl' }, label), el('span', { class: 'rv' }, value || '—'));

function nameInput(cell) {
  return el('input', { type: 'text', value: String(cell.value ?? ''), oninput: (ev) => {
    graph.getDataModel().setValue(cell, ev.target.value);
  } });
}

function tagsTab(cell) {
  const m = meta.get(cell.id);
  const e = editOf(cell.id);
  const list = [...(m.ea.tags || [])];
  for (const name of Object.keys(e.tags)) if (!list.some((x) => x.name === name)) list.push({ name, value: '', values: [], added: true });
  const box = el('div', { class: 'tags' });
  box.append(el('p', { class: 'hint' }, badge('model'), ' ', t.scope_model_hint));
  if (!list.length) box.append(el('p', { class: 'hint' }, t.no_tags));
  for (const tag of list) {
    const removed = e.tags[tag.name] === null;
    const value = tag.name in e.tags && !removed ? e.tags[tag.name] : (tag.value ?? '');
    const on = (v) => { if (v === (tag.value ?? '') && !tag.added) delete e.tags[tag.name]; else e.tags[tag.name] = v; refreshBpmn(cell); };
    let control;
    if (tag.values && tag.values.length) {
      control = el('select', { onchange: (ev) => on(ev.target.value) });
      const opts = tag.values.includes(value) ? tag.values : [value, ...tag.values];
      for (const v of opts) { const o = el('option', { value: v }, v || '—'); if (v === value) o.selected = true; control.append(o); }
    } else {
      control = el('input', { type: 'text', value: String(value), oninput: (ev) => on(ev.target.value) });
    }
    if (removed) control.disabled = true;
    const x = el('button', { class: 'x', title: t.remove_tag, onclick: (ev) => {
      ev.preventDefault();
      if (removed) { if (tag.added) e.tags[tag.name] = ''; else delete e.tags[tag.name]; } else if (tag.added) delete e.tags[tag.name]; else e.tags[tag.name] = null;
      render();
    } }, removed ? '↺' : '×');
    box.append(el('div', { class: `tag${removed ? ' removed' : ''}` }, el('span', { class: 'tn', title: tag.name }, tag.name), control, x));
  }
  const name = el('input', { type: 'text', placeholder: t.tag_name });
  const value = el('input', { type: 'text', placeholder: t.tag_value });
  box.append(el('div', { class: 'tag add' }, name, value, el('button', { onclick: (ev) => {
    ev.preventDefault();
    const n = name.value.trim();
    if (!n) return;
    e.tags[n] = value.value;
    render();
  } }, t.add_tag)));
  return box;
}

function colorRow(label, scope, value, onChange) {
  const picker = el('input', { type: 'color', value: value || '#FFFFFF' });
  const state = el('span', { class: 'rv' }, value || '');
  picker.addEventListener('input', () => { state.textContent = picker.value.toUpperCase(); onChange(picker.value.toUpperCase()); });
  const reset = el('button', { onclick: (ev) => { ev.preventDefault(); state.textContent = ''; picker.value = '#FFFFFF'; onChange(-1); } }, t.default_color);
  return row(label, scope, el('div', { class: 'color' }, picker, state, reset));
}

function appearanceTab(cell) {
  const m = meta.get(cell.id);
  const e = editOf(cell.id);
  const box = el('div');
  if (cell.isVertex()) {
    const shown = (v) => (v === -1 ? '' : v);
    box.append(colorRow(t.fill_diagram, 'diagram', shown(e.bgcolor ?? (m.ea.props || {}).bgcolor), (v) => {
      e.bgcolor = v;
      graph.setCellStyles('fillColor', v === -1 ? ((m.ea.props || {}).color || vertexStyle(m.ea).fillColor) : v, [cell]);
    }));
    box.append(colorRow(t.fill_model, 'model', shown(e.fields.color ?? (m.ea.props || {}).color), (v) => { e.fields.color = v; }));
  } else if (m.isNew) {
    box.append(el('p', { class: 'hint' }, t.after_save));
  } else {
    const s = el('select', { onchange: (ev) => { e.line_style = ev.target.value; } });
    const value = e.line_style ?? (m.ea.props || {}).line_style ?? '';
    if (!value) s.append(el('option', { value: '' }, '—'));
    for (const v of LINE_STYLES) { const o = el('option', { value: v }, t[`ls_${v}`]); if (v === value) o.selected = true; s.append(o); }
    box.append(row(t.line_style, 'diagram', s));
  }
  return box;
}

// Class members: a working copy per element; on saving it is compared with what the model has.
const CLASSIFIERS = ['Class', 'Interface', 'Enumeration', 'DataType', 'Signal', 'Component'];
const VIS = [['Public', '+'], ['Private', '-'], ['Protected', '#'], ['Package', '~']];
const members = new Map(); // cell id -> {attributes: [...], operations: [...]}
function membersOf(cell) {
  if (!members.has(cell.id)) {
    const ea = meta.get(cell.id).ea;
    const copy = (list) => (list || []).map((x) => ({ ...x, orig: { ...x } }));
    members.set(cell.id, { attributes: copy(ea.attribute_list), operations: copy(ea.operation_list) });
  }
  return members.get(cell.id);
}
const sign = (v) => (VIS.find((x) => x[0] === v) || VIS[0])[1];
function relabel(cell) {
  const ea = meta.get(cell.id).ea;
  const w = membersOf(cell);
  ea.attributes = w.attributes.map((a) => `${sign(a.visibility)} ${a.name}${a.type ? `: ${a.type}` : ''}`);
  ea.operations = w.operations.map((o) => `${sign(o.visibility)} ${o.name}(${o.parameters || ''})${o.type && o.type !== 'void' ? `: ${o.type}` : ''}`);
  // The box grows so that every attribute and operation stays visible (it never shrinks by itself).
  const stereo = ea.type === 'Interface' || (ea.stereotype && !BPMN.has(ea.stereotype));
  const height = (stereo ? 44 : 30) + 6 + ea.attributes.length * 15 + 6 + ea.operations.length * 15 + 8;
  const width = Math.min(380, Math.max(...[...ea.attributes, ...ea.operations].map((r) => r.length * 6.6 + 18), 0));
  const g = cell.geometry;
  if (g.height < height || g.width < width) {
    const ng = g.clone();
    ng.height = Math.max(g.height, height);
    ng.width = Math.max(g.width, width);
    graph.getDataModel().setGeometry(cell, ng);
  }
  graph.refresh(cell);
}
function membersTab(cell) {
  const w = membersOf(cell);
  const box = el('div', { class: 'members' });
  box.append(el('p', { class: 'hint' }, badge('model'), ' ', t.scope_model_hint));
  const section = (kind, title, list) => {
    box.append(el('h3', {}, title));
    if (!list.length) box.append(el('p', { class: 'hint' }, t.none));
    list.forEach((m, index) => {
      const vis = el('select', { title: t.visibility, onchange: (ev) => { m.visibility = ev.target.value; relabel(cell); } });
      for (const [v, s] of VIS) { const o = el('option', { value: v }, s); if (v === (m.visibility || 'Public')) o.selected = true; vis.append(o); }
      const name = el('input', { type: 'text', value: m.name || '', placeholder: t.f_name, oninput: (ev) => { m.name = ev.target.value; relabel(cell); } });
      const type = el('input', { type: 'text', value: m.type || '', placeholder: kind === 'attribute' ? t.f_type : t.return_type, oninput: (ev) => { m.type = ev.target.value; relabel(cell); } });
      const x = el('button', { class: 'x', title: t.remove_member, onclick: (ev) => { ev.preventDefault(); list.splice(index, 1); relabel(cell); render(); } }, '×');
      box.append(el('div', { class: 'member' }, vis, name, type, x));
      if (kind === 'operation') {
        const params = el('input', { type: 'text', value: m.parameters || '', placeholder: t.parameters_hint, oninput: (ev) => { m.parameters = ev.target.value; relabel(cell); } });
        if (m.id) { params.disabled = true; params.title = t.parameters_existing; }
        box.append(el('div', { class: 'member params' }, el('span', { class: 'fl' }, t.parameters), params));
      }
    });
    box.append(el('button', { class: 'addm', onclick: (ev) => {
      ev.preventDefault();
      list.push({ name: kind === 'attribute' ? t.new_attribute : t.new_operation, type: '', visibility: kind === 'attribute' ? 'Private' : 'Public', parameters: '' });
      relabel(cell);
      render();
    } }, kind === 'attribute' ? t.add_attribute : t.add_operation));
  };
  section('attribute', t.attributes, w.attributes);
  section('operation', t.operations, w.operations);
  return box;
}

function render() {
  panel.replaceChildren();
  const sel = graph.getSelectionCells().filter((c) => meta.get(c.id));
  panel.append(el('h2', {}, t.properties));
  if (sel.length !== 1) {
    panel.append(el('p', { class: 'hint' }, sel.length ? t.selected_many.replace('{0}', sel.length) : t.nothing_selected));
    panel.append(el('p', { class: 'legend' }, badge('model'), ' ', t.scope_model_hint), el('p', { class: 'legend' }, badge('diagram'), ' ', t.scope_diagram_hint));
    return;
  }
  const cell = sel[0];
  const m = meta.get(cell.id);
  const type = isMessage(cell) ? t.message : m.ea.stereotype ? `${m.ea.type} «${String(m.ea.stereotype).replace(/^.*::/, '')}»` : m.ea.type;
  panel.append(el('p', { class: 'kind' }, type));
  // What a change of the model reaches: the other diagrams that show this object.
  if (!m.isNew) {
    const others = m.ea.other_diagrams || [];
    const text = others.length ? t.used_on.replace('{0}', others.length).replace('{1}', others.slice(0, 5).join(', ') + (others.length > 5 ? '…' : '')) : t.used_nowhere;
    panel.append(el('p', { class: 'hint usage' }, badge('model'), ' ', text));
  }
  const tabs = el('div', { class: 'tabs' });
  const tabList = [['general', t.tab_general]];
  if (cell.isVertex() && CLASSIFIERS.includes(m.ea.type)) tabList.push(['members', t.tab_members]);
  tabList.push(['tags', t.tab_tags]);
  if (!isMessage(cell)) tabList.push(['appearance', t.tab_appearance]);
  if (!tabList.some(([k]) => k === tab)) tab = 'general';
  for (const [k, label] of tabList) {
    tabs.append(el('button', { class: k === tab ? 'on' : '', onclick: () => { tab = k; render(); } }, label));
  }
  panel.append(tabs);
  const body = el('div', { class: 'body' });
  if (tab === 'tags') body.append(tagsTab(cell));
  else if (tab === 'members') body.append(membersTab(cell));
  else if (tab === 'appearance') body.append(appearanceTab(cell));
  else if (cell.isVertex()) {
    body.append(row(t.f_name, 'model', nameInput(cell)));
    if (bpmnTypeTag(m.ea)) body.append(bpmnTypeRow(cell));
    for (const [field, label, kind] of EL_FIELDS) body.append(row(t[label], 'model', input(cell, field, kind)));
    const p = m.ea.props || {};
    body.append(readonly(t.f_package, p.package), readonly(t.f_guid, p.guid), readonly(t.f_created, p.created), readonly(t.f_modified, p.modified));
  } else if (isMessage(cell)) {
    body.append(row(t.f_name, 'model', nameInput(cell)));
    // The operations of the receiver: choosing one names the message after it.
    const ops = (meta.get(cell.target.id)?.ea.operation_list || []).map((o) => `${o.name}(${o.parameters || ''})`);
    if (ops.length) {
      const pick = el('select', { onchange: (ev) => { if (ev.target.value) { graph.getDataModel().setValue(cell, ev.target.value); render(); } } });
      pick.append(el('option', { value: '' }, '—'), ...ops.map((o) => el('option', { value: o }, o)));
      body.append(row(t.operation_of_target, 'model', pick));
    }
    const i = seqOrder.indexOf(cell);
    const up = el('button', { title: `${t.earlier} (Alt+↑)`, onclick: (ev) => { ev.preventDefault(); moveMessage(cell, -1); } }, '▲');
    const down = el('button', { title: `${t.later} (Alt+↓)`, onclick: (ev) => { ev.preventDefault(); moveMessage(cell, 1); } }, '▼');
    up.disabled = i <= 0;
    down.disabled = i >= seqOrder.length - 1;
    body.append(row(t.f_order, 'model', el('div', { class: 'order' }, el('b', {}, `${i + 1} / ${seqOrder.length}`), up, down)));
    body.append(row(t.f_message_sort, 'model', input(cell, 'message_sort', 'sort')));
    body.append(row(t.f_is_return, 'model', input(cell, 'is_return', 'bool')));
    body.append(readonly(t.f_source, String(cell.source?.value ?? '')), readonly(t.f_target, String(cell.target?.value ?? '')));
    body.append(row(t.f_notes, 'model', input(cell, 'notes', 'text')));
  } else {
    body.append(row(t.f_name, 'model', nameInput(cell)));
    const ends = (c) => String(c?.value ?? '');
    body.append(readonly(t.f_source, ends(cell.source)), readonly(t.f_target, ends(cell.target)));
    for (const [field, label, kind] of CONN_FIELDS) body.append(row(t[label], 'model', input(cell, field, kind)));
    if ((m.ea.props || {}).guid) body.append(readonly(t.f_guid, m.ea.props.guid));
  }
  panel.append(body);
}
document.body.append(el('datalist', { id: 'multiplicities' }, MULTIPLICITIES.map((v) => el('option', { value: v }))));
graph.getSelectionModel().addListener(InternalEvent.CHANGE, () => { if (!graph.isEditing()) render(); });
render();

// --- free spot for a new element: near the middle of the view, not over other elements -----------------------------

function freeSpot(w, h) {
  const s = graph.getView().scale;
  const tr = graph.getView().translate;
  const cx = Math.round((container.scrollLeft + container.clientWidth / 2) / s - tr.x - w / 2);
  const cy = Math.round((container.scrollTop + container.clientHeight / 2) / s - tr.y - h / 2);
  const boxes = Object.values(graph.getDataModel().cells).filter((c) => c.isVertex() && c.geometry).map((c) => c.geometry);
  const free = (x, y) => boxes.every((g) => x + w + 20 <= g.x || g.x + g.width + 20 <= x || y + h + 20 <= g.y || g.y + g.height + 20 <= y);
  // Rings around the middle, on the 10 px grid.
  for (let r = 0; r <= 1200; r += 30) {
    for (let a = 0; a < 360; a += r ? Math.max(8, 900 / r) : 360) {
      const x = Math.round((cx + r * Math.cos((a * Math.PI) / 180)) / 10) * 10;
      const y = Math.round((cy + r * Math.sin((a * Math.PI) / 180)) / 10) * 10;
      if (x >= 0 && y >= 0 && free(x, y)) return [x, y];
    }
  }
  return [Math.max(0, cx), Math.max(0, cy)];
}

// --- existing elements of the model put on the diagram ------------------------------------------------------------

const SIZES = { Actor: [40, 80], UseCase: [150, 70], Event: [30, 30], Decision: [40, 40], Note: [160, 60], Activity: [110, 60], Action: [110, 50] };
const insertBox = document.getElementById('insert');
const insertList = insertBox.querySelector('.i-list');
const insertInput = insertBox.querySelector('input');
const placedIds = () => new Set([...meta.values()].filter((m) => m.kind === 'element').map((m) => m.ea.id));
function showCandidates() {
  const q = insertInput.value.trim().toLowerCase();
  const taken = placedIds();
  const hits = data.candidates.filter((c) => !taken.has(c.id) && (!q || `${c.name} ${c.type} ${c.stereotype} ${c.package}`.toLowerCase().includes(q)));
  // Names that start with the text first, then names that contain it, then the rest (type, package).
  const rank = (c) => { const n = c.name.toLowerCase(); return !q ? 0 : n.startsWith(q) ? 0 : n.includes(q) ? 1 : 2; };
  hits.sort((a, b) => rank(a) - rank(b));
  insertList.replaceChildren();
  for (const c of hits.slice(0, 200)) {
    const kind = c.stereotype ? `${c.type} «${String(c.stereotype).replace(/^.*::/, '')}»` : c.type;
    insertList.append(el('button', { class: 'i-row', onclick: () => placeExisting(c) }, el('b', {}, c.name), el('span', {}, `${kind} · ${c.package || ''}`)));
  }
  if (!hits.length) insertList.append(el('p', { class: 'hint' }, t.insert_none));
  else if (hits.length > 200) insertList.append(el('p', { class: 'hint' }, t.insert_more.replace('{0}', '200')));
}
function placeExisting(c) {
  const [w, h] = SIZES[c.type] || (c.stereotype && /Event$/.test(c.stereotype) ? [30, 30] : [160, 80]);
  const [x, y] = freeSpot(w, h);
  const ea = { ...c, attributes: [], operations: [], attribute_list: [], operation_list: [], props: {}, tags: [], placed: true };
  let v;
  graph.batchUpdate(() => {
    v = graph.insertVertex({ parent, id: `p${c.id}`, value: c.name, position: [x, y], size: [w, h], style: vertexStyle(ea) });
    meta.set(v.id, { kind: 'element', ea, isNew: false, placed: true });
  });
  graph.setSelectionCell(v);
  insertBox.hidden = true;
}
document.getElementById('b-insert').addEventListener('click', () => {
  insertBox.hidden = false;
  insertInput.value = '';
  showCandidates();
  insertInput.focus();
});
insertInput.addEventListener('input', showCandidates);
insertBox.querySelector('.i-close').addEventListener('click', () => { insertBox.hidden = true; });
insertBox.addEventListener('keydown', (ev) => { if (ev.key === 'Escape') insertBox.hidden = true; });

// --- sequence diagrams: lifelines and messages in their order ----------------------------------------------------
// EA lays messages out from top to bottom by their number (SequenceNo), so here too the order is what counts: the
// vertical place of a message follows from its position in the list, which the user changes with ▲ ▼ (Alt+↑/↓).

function lifelines() {
  return Object.values(graph.getDataModel().cells).filter((c) => c.isVertex() && isLifeline(meta.get(c.id)?.ea));
}
function messageStyle(cell) {
  const reply = !!current(cell, 'is_return');
  const asynchronous = current(cell, 'message_sort') === 'Asynchronous';
  return {
    strokeColor: '#3B4B59', fontSize: 11, labelBackgroundColor: '#FFFFFF', dashed: reply,
    endArrow: reply || asynchronous ? 'open' : 'block', endFill: !(reply || asynchronous), movable: false, bendable: false,
    exitPerimeter: false, entryPerimeter: false, verticalAlign: 'bottom', spacingBottom: 2,
  };
}
// Lays the messages out in their order and makes the lifelines long enough for them.
function layoutMessages() {
  if (!SEQ) return;
  const present = graph.getDataModel().cells;
  for (let i = seqOrder.length - 1; i >= 0; i--) if (!present[seqOrder[i].id]) seqOrder.splice(i, 1);
  const lines = lifelines();
  const top = lines.length ? Math.min(...lines.map((c) => c.geometry.y)) : 40;
  const first = top + HEADER + 30;
  const needed = first + STEP * Math.max(seqOrder.length, 1) + 30;
  graph.batchUpdate(() => {
    for (const c of lines) {
      if (c.geometry.y + c.geometry.height < needed) {
        const g = c.geometry.clone();
        g.height = needed - g.y;
        graph.getDataModel().setGeometry(c, g);
      }
    }
    seqOrder.forEach((e, i) => {
      const y = first + STEP * i;
      const s = e.source.geometry;
      const d = e.target.geometry;
      const style = { ...messageStyle(e), exitX: 0.5, exitY: (y - s.y) / s.height, entryX: 0.5 };
      const g = e.geometry.clone();
      if (e.source === e.target) {
        // A message to itself: out to the right and back a little lower.
        const x = s.x + s.width / 2 + 40;
        style.entryY = (y + 18 - s.y) / s.height;
        g.points = [new Point(x, y), new Point(x, y + 18)];
      } else {
        style.entryY = (y - d.y) / d.height;
        g.points = [];
      }
      graph.getDataModel().setStyle(e, style);
      graph.getDataModel().setGeometry(e, g);
    });
  });
}
function moveMessage(cell, delta) {
  const i = seqOrder.indexOf(cell);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= seqOrder.length) return;
  [seqOrder[i], seqOrder[j]] = [seqOrder[j], seqOrder[i]];
  layoutMessages();
  render();
}
if (SEQ) {
  const byOrder = (a, b) => {
    const pa = meta.get(a.id).ea.props || {};
    const pb = meta.get(b.id).ea.props || {};
    return (pa.seq || 0) - (pb.seq || 0) || (pa.y || 0) - (pb.y || 0) || meta.get(a.id).ea.id - meta.get(b.id).ea.id;
  };
  for (const c of Object.values(graph.getDataModel().cells)) {
    const m = meta.get(c.id);
    if (!isMessage(c)) continue;
    m.ea.props = m.ea.props || {};
    m.ea.props.message_sort = m.ea.props.sort || 'Synchronous';
    m.ea.props.is_return = !!m.ea.props.return;
    seqOrder.push(c);
  }
  seqOrder.sort(byOrder);
  // Lifelines move only sideways: their tops stay level, as in EA.
  graph.addListener(InternalEvent.CELLS_MOVED, (_s, evt) => {
    const moved = (evt.getProperty('cells') || []).filter((c) => c.isVertex() && isLifeline(meta.get(c.id)?.ea));
    if (!moved.length) return;
    const others = lifelines().filter((c) => !moved.includes(c));
    const top = others.length ? Math.min(...others.map((c) => c.geometry.y)) : moved[0].geometry.y;
    graph.batchUpdate(() => moved.forEach((c) => { const g = c.geometry.clone(); g.y = top; graph.getDataModel().setGeometry(c, g); }));
    layoutMessages();
  });
  graph.addListener(InternalEvent.CELLS_RESIZED, () => layoutMessages());
  graph.addListener(InternalEvent.CELLS_REMOVED, () => layoutMessages());
  document.getElementById('status').textContent = t.seq_hint;
  layoutMessages();
}

// --- BPMN: the kind of a task, event or gateway (EA keeps it in tagged values of the BPMN profile) ---------------

const bpmnTypeTag = (ea) => {
  const st = String(ea?.stereotype || '').replace(/^.*::/, '');
  if (st === 'Activity') return 'taskType';
  if (/Event$/.test(st)) return 'eventDefinition';
  if (st === 'Gateway') return 'gatewayType';
  return null;
};
const BPMN_VALUES = {
  taskType: ['Abstract', 'User', 'Service', 'Manual', 'Script', 'Send', 'Receive', 'BusinessRule'],
  eventDefinition: ['None', 'Message', 'Timer', 'Signal', 'Error', 'Escalation', 'Conditional', 'Compensation', 'Cancel', 'Terminate', 'Multiple', 'ParallelMultiple'],
  gatewayType: ['Exclusive', 'Inclusive', 'Parallel', 'Complex', 'Event'],
};
// The value of a tagged value: what the user chose in the editor, or what the model has.
function tagValue(cell, name) {
  const e = edits.get(cell.id);
  if (e && name in e.tags && e.tags[name] !== null) return e.tags[name];
  const tag = (meta.get(cell.id)?.ea.tags || []).find((x) => x.name === name);
  return tag ? (tag.value ?? '') : '';
}
const svgImage = (body, size, fill = 'none') => new ImageBox('data:image/svg+xml;utf8,' + encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="${size}" height="${size}" fill="${fill}" stroke="#3B4B59" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`,
), size, size);
const TASK_ICONS = {
  User: '<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4.5 3.6-7 8-7s8 2.5 8 7"/>',
  Service: '<circle cx="12" cy="12" r="3.2"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>',
  Manual: '<path d="M7 12V6a1.5 1.5 0 013 0v5M10 11V4a1.5 1.5 0 013 0v7M13 11V5a1.5 1.5 0 013 0v7M16 12V8a1.5 1.5 0 013 0v6a7 7 0 01-7 7h-1a6 6 0 01-5-2.8l-3-5a1.5 1.5 0 012.6-1.5L7 15"/>',
  Script: '<path d="M7 3h11v18H7z"/><path d="M10 8h5M10 12h5M10 16h5"/>',
  Send: '<rect x="3" y="6" width="18" height="12" fill="#3B4B59"/><path d="M3 6l9 7 9-7" stroke="#FFFFFF"/>',
  Receive: '<rect x="3" y="6" width="18" height="12"/><path d="M3 6l9 7 9-7"/>',
  BusinessRule: '<rect x="3" y="5" width="18" height="14"/><path d="M3 9.5h18M8 9.5V19"/>',
};
const EVENT_ICONS = {
  Message: '<rect x="4" y="7" width="16" height="10"/><path d="M4 7l8 6 8-6"/>',
  Timer: '<circle cx="12" cy="12" r="8"/><path d="M12 7v5l3 2"/>',
  Signal: '<path d="M12 4.5l8 14.5H4z"/>',
  Error: '<path d="M5 19l4-11 4 6 6-10-4 13-4-6z"/>',
  Escalation: '<path d="M12 4l6 15-6-5-6 5z"/>',
  Conditional: '<rect x="6" y="4" width="12" height="16"/><path d="M8.5 8h7M8.5 12h7M8.5 16h7"/>',
  Compensation: '<path d="M11 6l-7 6 7 6zM20 6l-7 6 7 6z"/>',
  Cancel: '<path d="M6.5 6.5l11 11M17.5 6.5l-11 11" stroke-width="3"/>',
  Terminate: '<circle cx="12" cy="12" r="7" fill="#3B4B59"/>',
  Multiple: '<path d="M12 4l8 6-3 9H7l-3-9z"/>',
  ParallelMultiple: '<path d="M12 4v16M4 12h16" stroke-width="3"/>',
};
const GATEWAY_MARKS = {
  Exclusive: '<path d="M7 7l10 10M17 7L7 17" stroke-width="3"/>',
  Parallel: '<path d="M12 5v14M5 12h14" stroke-width="3"/>',
  Inclusive: '<circle cx="12" cy="12" r="6" stroke-width="2.5"/>',
  Complex: '<path d="M12 5v14M5 12h14M7 7l10 10M17 7L7 17" stroke-width="2.4"/>',
  Event: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="6.3"/><path d="M12 8.5l3.4 2.5-1.3 4h-4.2l-1.3-4z"/>',
};
const LOOP_MARKS = {
  Standard: '<path d="M8 17a6 6 0 118 0"/><path d="M16 13v4h4"/>',
  Parallel: '<path d="M8 6v12M12 6v12M16 6v12" stroke-width="2"/>',
  Sequential: '<path d="M6 8h12M6 12h12M6 16h12" stroke-width="2"/>',
};
// Draws the kind as in BPMN: an icon in the corner of a task, a symbol inside an event, a marker inside a gateway.
function refreshBpmn(cell) {
  if (!cell?.isVertex()) return;
  const m = meta.get(cell.id);
  const tag = bpmnTypeTag(m?.ea);
  graph.removeCellOverlays(cell);
  if (!tag) return;
  const g = cell.geometry;
  const add = (image, align, vAlign, dx, dy) => graph.addCellOverlay(cell, new CellOverlay(image, null, align, vAlign, new Point(dx, dy), 'default'));
  const value = tagValue(cell, tag);
  const end = /End/.test(String(m.ea.stereotype));
  if (tag === 'taskType') {
    if (TASK_ICONS[value]) add(svgImage(TASK_ICONS[value], 16), 'left', 'top', 13, 12);
    const loop = tagValue(cell, 'loopCharacteristics');
    if (loop === 'Standard') add(svgImage(LOOP_MARKS.Standard, 14), 'center', 'bottom', 0, -9);
    if (loop === 'MultiInstance') add(svgImage(tagValue(cell, 'isSequential') === 'true' ? LOOP_MARKS.Sequential : LOOP_MARKS.Parallel, 14), 'center', 'bottom', 0, -9);
  } else if (tag === 'eventDefinition' && EVENT_ICONS[value]) {
    add(svgImage(EVENT_ICONS[value], Math.max(10, Math.round(Math.min(g.width, g.height) * 0.6)), end && value !== 'Terminate' ? '#3B4B59' : 'none'), 'center', 'middle', 0, 0);
  } else if (tag === 'gatewayType') {
    const mark = value === 'Exclusive' || !value ? (tagValue(cell, 'markerVisible') === 'true' ? GATEWAY_MARKS.Exclusive : null) : GATEWAY_MARKS[value];
    if (mark) add(svgImage(mark, Math.max(10, Math.round(Math.min(g.width, g.height) * 0.55))), 'center', 'middle', 0, 0);
  }
  // The marks never catch the mouse: the element under them stays easy to select and drag.
  graph.getView().getState(cell)?.overlays?.forEach((shape) => { if (shape?.node) shape.node.style.pointerEvents = 'none'; });
}
const refreshAllBpmn = () => Object.values(graph.getDataModel().cells).forEach((c) => { if (c.isVertex() && bpmnTypeTag(meta.get(c.id)?.ea)) refreshBpmn(c); });
graph.getView().addListener(InternalEvent.SCALE, refreshAllBpmn);
graph.getView().addListener(InternalEvent.SCALE_AND_TRANSLATE, refreshAllBpmn);
graph.addListener(InternalEvent.CELLS_RESIZED, refreshAllBpmn);
refreshAllBpmn();

// The kind of a BPMN element in the General tab: a pick-list bound to its tagged value.
function bpmnTypeRow(cell) {
  const tag = bpmnTypeTag(meta.get(cell.id).ea);
  const def = (meta.get(cell.id).ea.tags || []).find((x) => x.name === tag);
  const values = def?.values?.length ? def.values : BPMN_VALUES[tag];
  const value = tagValue(cell, tag) || values[0];
  const s = el('select', { onchange: (ev) => {
    const e = editOf(cell.id);
    if (ev.target.value === (def?.value ?? '')) delete e.tags[tag]; else e.tags[tag] = ev.target.value;
    // An exclusive gateway shows its X when the marker is on.
    if (tag === 'gatewayType' && ev.target.value === 'Exclusive' && tagValue(cell, 'markerVisible') !== 'true') e.tags.markerVisible = 'true';
    refreshBpmn(cell);
  } });
  for (const v of values) { const o = el('option', { value: v }, t[`bpmn_${v}`] || v); if (v === value) o.selected = true; s.append(o); }
  return row(t[`bpmn_${tag}`], 'model', s);
}

// --- connecting: a blue arrow on the element under the mouse, dragged to another element ---------------------------

const connector = graph.getPlugin('ConnectionHandler');
connector.connectImage = svgImage('<circle cx="12" cy="12" r="11" fill="#0E4A6B" stroke="none"/><path d="M6.5 12h10M12.5 7.5l4.5 4.5-4.5 4.5" stroke="#FFFFFF" stroke-width="2.4"/>', 22);
connector.marker.hotspot = 1;
// maxGraph inserts every new edge with the id '' (ConnectionHandler.connect): each one gets its own here, so the
// properties and the saving of two new relationships never mix up.
connector.insertEdge = (parent, _id, value, source, target, style) => graph.insertEdge({ parent, id: `k${++counter}`, value, source, target, style });
connector.getIconPosition = (icon, state) => {
  const s = graph.getView().scale;
  const top = isLifeline(meta.get(state.cell.id)?.ea) ? state.y + 25 * s : state.getCenterY();
  return new Point(state.x + state.width - icon.bounds.width / 2, top - icon.bounds.height / 2);
};

// --- toolbox and new connectors ----------------------------------------------------------------------------------

let counter = 0;
const toolbox = document.getElementById('toolbox');
for (const tool of data.toolbox) {
  const b = document.createElement('button');
  b.textContent = tool.label;
  b.title = t.add_hint;
  b.addEventListener('click', () => {
    let [x, y] = freeSpot(tool.width, tool.height);
    let height = tool.height;
    if (SEQ && isLifeline({ type: tool.type })) {
      const lines = lifelines();
      if (lines.length) {
        x = Math.max(...lines.map((c) => c.geometry.x + c.geometry.width)) + 40;
        y = Math.min(...lines.map((c) => c.geometry.y));
        height = Math.max(...lines.map((c) => c.geometry.height));
      }
    }
    const ea = { type: tool.type, stereotype: tool.stereotype || '', subtype: tool.subtype || 0, attributes: [], operations: [], props: {}, tags: [], attribute_list: [], operation_list: [] };
    let v;
    graph.batchUpdate(() => {
      v = graph.insertVertex({ parent, id: `n${++counter}`, value: tool.name, position: [x, y], size: [tool.width, height], style: vertexStyle(ea) });
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
// The message selected before a new one is drawn: the new one goes right after it.
let lastMessage = null;
graph.getSelectionModel().addListener(InternalEvent.CHANGE, () => { const c = graph.getSelectionCell(); if (isMessage(c)) lastMessage = c; });
graph.getPlugin('ConnectionHandler').addListener(InternalEvent.CONNECT, (_s, evt) => {
  const edge = evt.getProperty('cell');
  const ct = data.connector_types[Number(connType.value) || 0];
  const ea = { type: ct.type, stereotype: ct.stereotype || '', name: '', props: {}, tags: [] };
  if (SEQ && ct.type === 'Sequence') {
    ea.props = { message_sort: ct.sort || 'Synchronous', is_return: !!ct.reply };
    meta.set(edge.id, { kind: 'connector', ea, isNew: true });
    // After the selected message, else at the end.
    const after = seqOrder.indexOf(lastMessage);
    seqOrder.splice(after >= 0 ? after + 1 : seqOrder.length, 0, edge);
    layoutMessages();
    graph.setSelectionCell(edge);
    return;
  }
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
// The route of an edge as the editor draws it, in diagram coordinates: bend points, and where it starts and ends.
function drawnRoute(cell) {
  const state = graph.getView().getState(cell);
  const abs = state?.absolutePoints;
  if (!abs || abs.length < 2 || abs.some((p) => !p)) return null;
  const s = graph.getView().scale;
  const tr = graph.getView().translate;
  const at = (p) => [Math.round(p.x / s - tr.x), Math.round(p.y / s - tr.y)];
  return { points: abs.slice(1, -1).map(at), start: at(abs[0]), end: at(abs[abs.length - 1]) };
}
function changes() {
  const out = {
    action: 'save', moved: [], renamed: [], routed: [], created: [], connected: [], removed: [], hidden: [], deleted: [], deleted_connectors: [],
    updated: [], updated_connectors: [], colors: [], line_styles: [], placed: [], sequence: [],
  };
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
  const newKey = (cell) => meta.get(cell.id)?.key || cell.id;
  const key = (cell) => (meta.get(cell.id)?.isNew ? `#${newKey(cell)}` : meta.get(cell.id).ea.id);
  for (const cell of Object.values(graph.getDataModel().cells)) {
    const m = meta.get(cell.id);
    if (!m) continue;
    if (cell.isVertex()) {
      const g = cell.geometry;
      const box = { left: round(g.x), top: round(g.y), width: round(g.width), height: round(g.height) };
      if (m.isNew) {
        out.created.push({ key: cell.id, name: String(cell.value || ''), type: m.ea.type, stereotype: m.ea.stereotype, subtype: m.ea.subtype || 0, ...box });
        continue;
      }
      if (m.placed) {
        out.placed.push({ id: m.ea.id, ...box });
        continue;
      }
      const o = original.get(cell.id);
      if (box.left !== o.x || box.top !== o.y || box.width !== o.w || box.height !== o.h) out.moved.push({ id: m.ea.id, ...box });
      if (String(cell.value) !== o.name) {
        if (isNote(m.ea)) out.updated.push({ id: m.ea.id, fields: { notes: String(cell.value) }, tags: {} });
        else out.renamed.push({ id: m.ea.id, name: String(cell.value) });
      }
    } else if (cell.isEdge()) {
      if (!cell.source || !cell.target) continue;
      const points = (cell.geometry.points || []).map((p) => [round(p.x), round(p.y)]);
      // The route as drawn (orthogonal lines are computed by the editor): EA gets exactly that, bends and touch points.
      const drawn = drawnRoute(cell);
      if (m.isNew) {
        const message = isMessage(cell)
          ? { sequence: seqOrder.indexOf(cell) + 1, message_sort: current(cell, 'message_sort'), is_return: !!current(cell, 'is_return') }
          : {};
        out.connected.push({ key: newKey(cell), type: m.ea.type, stereotype: m.ea.stereotype, name: String(cell.value || ''), source: key(cell.source), target: key(cell.target), points, ...message, ...(isMessage(cell) || !drawn ? {} : { route: drawn }) });
        continue;
      }
      // Messages: their place is their number (below), not a route.
      if (isMessage(cell)) {
        const n = seqOrder.indexOf(cell) + 1;
        if (n !== (m.ea.props || {}).seq) out.sequence.push({ id: m.ea.id, sequence: n });
        continue;
      }
      const o = original.get(cell.id);
      if (drawn && JSON.stringify(drawn.points) !== o.points) out.routed.push({ id: m.ea.id, ...drawn });
      else if (!drawn && JSON.stringify(points) !== o.points) out.routed.push({ id: m.ea.id, points });
    }
  }
  // Properties from the panel (new objects by their key: they are created first).
  for (const [id, e] of edits) {
    const cell = present[id];
    const m = meta.get(id);
    if (!cell || !m) continue;
    const ref = key(cell);
    const fields = { ...e.fields };
    if ('stereotype' in fields && fields.stereotype === '') fields.clear_stereotype = true;
    if (cell.isEdge() && !m.isNew && String(cell.value || '') !== original.get(id).name) fields.name = String(cell.value || '');
    const tags = { ...e.tags };
    if (Object.keys(fields).length || Object.keys(tags).length) (cell.isVertex() ? out.updated : out.updated_connectors).push({ id: ref, fields, tags });
    if (e.bgcolor !== undefined) out.colors.push({ id: ref, bgcolor: e.bgcolor });
    if (e.line_style && !m.isNew && e.line_style !== (m.ea.props || {}).line_style) out.line_styles.push({ id: ref, style: e.line_style });
  }
  // Class members: deleted, changed, added.
  out.members = [];
  for (const [id, w] of members) {
    const cell = present[id];
    const m = meta.get(id);
    if (!cell || !m) continue;
    for (const kind of ['attribute', 'operation']) {
      const now = kind === 'attribute' ? w.attributes : w.operations;
      const before = (kind === 'attribute' ? m.ea.attribute_list : m.ea.operation_list) || [];
      for (const b of before) if (!now.some((x) => x.id === b.id)) out.members.push({ kind, action: 'delete', id: b.id });
      for (const x of now) {
        if (!x.id) {
          if (x.name) out.members.push({ kind, action: 'create', element: key(cell), name: x.name, type: x.type, visibility: x.visibility, parameters: x.parameters });
          continue;
        }
        const diff = {};
        for (const f of ['name', 'type', 'visibility']) if ((x[f] || '') !== (x.orig[f] || '')) diff[f] = x[f] || '';
        if (Object.keys(diff).length) out.members.push({ kind, action: 'update', id: x.id, ...diff });
      }
    }
  }
  // Renamed existing connectors without other edits in the panel.
  for (const cell of Object.values(present)) {
    const m = meta.get(cell.id);
    if (!m || !cell.isEdge() || m.isNew || edits.has(cell.id) || !cell.source || !cell.target) continue;
    if (String(cell.value || '') !== original.get(cell.id).name) out.updated_connectors.push({ id: m.ea.id, fields: { name: String(cell.value || '') }, tags: {} });
  }
  return out;
}
function send(message) {
  window.ipc.postMessage(JSON.stringify(message));
}
document.getElementById('save').addEventListener('click', () => {
  const c = changes();
  const count = c.moved.length + c.renamed.length + c.routed.length + c.created.length + c.connected.length
    + c.removed.length + c.hidden.length + c.deleted.length + c.deleted_connectors.length
    + c.updated.length + c.updated_connectors.length + c.colors.length + c.line_styles.length + c.members.length + c.placed.length + c.sequence.length;
  if (!count) { document.getElementById('status').textContent = t.nothing; return; }
  send(c);
});
document.getElementById('cancel').addEventListener('click', () => send({ action: 'cancel' }));
// Tidy with AI: the changes so far are saved, then the assistant arranges the diagram (also with no changes).
document.getElementById('tidy').addEventListener('click', () => send({ ...changes(), tidy: true }));
document.getElementById('title').textContent = data.diagram.name;
graph.getPlugin('fit')?.fitCenter({ margin: 20 });
if (graph.getView().scale > 1) graph.zoomActual();

// Development self-test (LOONAR_EDITOR_SELFTEST, debug builds only): scripted edits, then saving.
if (data.selftest === 'props') {
  setTimeout(() => {
    const all = Object.values(graph.getDataModel().cells);
    const cust = all.find((c) => c.isVertex() && c.value === 'Customer');
    const edge = all.find((c) => c.isEdge() && meta.get(c.id) && !meta.get(c.id).isNew);
    graph.setSelectionCell(cust);
    for (const k of ['general', 'tags', 'appearance']) { tab = k; render(); }
    setField(cust, 'status', 'Approved');
    setField(cust, 'version', '2.0');
    setField(cust, 'phase', '2.0');
    setField(cust, 'abstract', true);
    setField(cust, 'notes', 'Customer of the shop (selftest).');
    editOf(cust.id).tags.owner = 'sales';
    editOf(cust.id).bgcolor = '#FFE599';
    graph.setSelectionCell(edge);
    for (const k of ['general', 'tags', 'appearance']) { tab = k; render(); }
    setField(edge, 'source_multiplicity', '1');
    setField(edge, 'target_multiplicity', '0..*');
    setField(edge, 'target_role', 'orders');
    setField(edge, 'direction', 'Source -> Destination');
    editOf(edge.id).tags.reviewed = 'yes';
    editOf(edge.id).line_style = 'orthogonal_square';
    document.getElementById('status').textContent = JSON.stringify(changes()).slice(0, 300);
    setTimeout(() => document.getElementById('save').click(), 1000);
  }, 1500);
} else if (data.selftest === 'route') {
  // Saving as drawn: every route goes to EA.
  setTimeout(() => document.getElementById('save').click(), 1500);
} else if (data.selftest === 'newrel') {
  // A new relationship with its direction set before saving (the way the connect arrow creates it).
  setTimeout(() => {
    const v = (n) => Object.values(graph.getDataModel().cells).find((c) => c.isVertex() && c.value === n);
    const e = connector.insertEdge(parent, '', '', v('Invoice'), v('Product'), {});
    meta.set(e.id, { kind: 'connector', ea: { type: 'Association', stereotype: '', props: {}, tags: [] }, isNew: true });
    setField(e, 'direction', 'Source -> Destination');
    setTimeout(() => document.getElementById('save').click(), 800);
  }, 1500);
} else if (data.selftest === 'types') {
  // Every element of the toolbox once, the first two joined with the first kind of connector.
  setTimeout(() => {
    const added = [];
    document.querySelectorAll('#toolbox button').forEach((b, i) => {
      b.click();
      const cell = graph.getSelectionCell();
      graph.stopEditing(false);
      if (!cell.value) graph.getDataModel().setValue(cell, `T${i + 1}`);
      added.push(cell);
    });
    const ct = data.connector_types[0];
    const e = graph.insertEdge({ parent, id: 'n99', value: '', source: added[0], target: added[1], style: edgeStyle(ct) });
    meta.set(e.id, { kind: 'connector', ea: { type: ct.type, stereotype: ct.stereotype || '', props: {}, tags: [] }, isNew: true });
    setTimeout(() => document.getElementById('save').click(), 1000);
  }, 1500);
} else if (data.selftest === 'seq') {
  setTimeout(() => {
    const all = () => Object.values(graph.getDataModel().cells);
    const msg = (n) => all().find((c) => isMessage(c) && c.value === n);
    // log() right after order(), confirm() renamed, pay() asynchronous.
    const log = msg('log()');
    while (seqOrder.indexOf(log) > 1) moveMessage(log, -1);
    graph.getDataModel().setValue(msg('confirm()'), 'confirmOrder()');
    setField(msg('pay()'), 'message_sort', 'Asynchronous');
    // A new lifeline and a message to it right after pay().
    document.querySelector('#toolbox button').click();
    const warehouse = graph.getSelectionCell();
    graph.stopEditing(false);
    graph.getDataModel().setValue(warehouse, 'Warehouse');
    const shop = all().find((c) => c.isVertex() && c.value === 'Shop');
    const e = graph.insertEdge({ parent, id: 'n99', value: 'reserve()', source: shop, target: warehouse, style: {} });
    meta.set(e.id, { kind: 'connector', ea: { type: 'Sequence', stereotype: '', props: { message_sort: 'Synchronous', is_return: false }, tags: [] }, isNew: true });
    seqOrder.splice(seqOrder.indexOf(msg('pay()')) + 1, 0, e);
    layoutMessages();
    document.getElementById('status').textContent = JSON.stringify(changes()).slice(0, 300);
    setTimeout(() => document.getElementById('save').click(), 1000);
  }, 1500);
} else if (data.selftest === 'members') {
  setTimeout(() => {
    const cust = Object.values(graph.getDataModel().cells).find((c) => c.isVertex() && c.value === 'Customer');
    graph.setSelectionCell(cust);
    tab = 'members';
    render();
    const w = membersOf(cust);
    w.attributes[0].type = 'Text';
    w.attributes.splice(1, 1);
    w.attributes.push({ name: 'phone', type: 'String', visibility: 'Private', parameters: '' });
    w.operations.splice(0, 1);
    w.operations.push({ name: 'discount', type: 'Money', visibility: 'Public', parameters: 'order: Order, code' });
    relabel(cust);
    render();
    setTimeout(() => document.getElementById('save').click(), 1000);
  }, 1500);
} else if (data.selftest) {
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

// Handle for scripted checks of the page (screenshots of the panel and dialogs in a browser).
window.__editor = { graph, render, askDelete, changes, setField, meta, setTab: (k) => { tab = k; render(); } };
