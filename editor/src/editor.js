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
  // Typing in the properties panel is not an editor command.
  if (graph.isEditing() || ev.target.closest?.('input, textarea, select')) return;
  if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === 'z') { undo.undo(); ev.preventDefault(); }
  if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === 'y') { undo.redo(); ev.preventDefault(); }
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

const edits = new Map(); // cell id -> {fields: {...}, tags: {name: value|null}, bgcolor?, line_style?}
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
  const on = (v) => setField(cell, field, v);
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
    const on = (v) => { if (v === (tag.value ?? '') && !tag.added) delete e.tags[tag.name]; else e.tags[tag.name] = v; };
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
  const type = m.ea.stereotype ? `${m.ea.type} «${String(m.ea.stereotype).replace(/^.*::/, '')}»` : m.ea.type;
  panel.append(el('p', { class: 'kind' }, type));
  const tabs = el('div', { class: 'tabs' });
  const tabList = [['general', t.tab_general]];
  if (cell.isVertex() && CLASSIFIERS.includes(m.ea.type)) tabList.push(['members', t.tab_members]);
  tabList.push(['tags', t.tab_tags], ['appearance', t.tab_appearance]);
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
    for (const [field, label, kind] of EL_FIELDS) body.append(row(t[label], 'model', input(cell, field, kind)));
    const p = m.ea.props || {};
    body.append(readonly(t.f_package, p.package), readonly(t.f_guid, p.guid), readonly(t.f_created, p.created), readonly(t.f_modified, p.modified));
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

// --- toolbox and new connectors ----------------------------------------------------------------------------------

let counter = 0;
const toolbox = document.getElementById('toolbox');
for (const tool of data.toolbox) {
  const b = document.createElement('button');
  b.textContent = tool.label;
  b.title = t.add_hint;
  b.addEventListener('click', () => {
    const [x, y] = freeSpot(tool.width, tool.height);
    const ea = { type: tool.type, stereotype: tool.stereotype || '', attributes: [], operations: [], props: {}, tags: [], attribute_list: [], operation_list: [] };
    let v;
    graph.batchUpdate(() => {
      v = graph.insertVertex({ parent, id: `n${++counter}`, value: tool.name, position: [x, y], size: [tool.width, tool.height], style: vertexStyle(ea) });
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
  const ea = { type: ct.type, stereotype: ct.stereotype || '', name: '', props: {}, tags: [] };
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
  const out = {
    action: 'save', moved: [], renamed: [], routed: [], created: [], connected: [], removed: [], hidden: [], deleted: [], deleted_connectors: [],
    updated: [], updated_connectors: [], colors: [], line_styles: [], placed: [],
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
      if (m.placed) {
        out.placed.push({ id: m.ea.id, ...box });
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
    + c.updated.length + c.updated_connectors.length + c.colors.length + c.line_styles.length + c.members.length + c.placed.length;
  if (!count) { document.getElementById('status').textContent = t.nothing; return; }
  send(c);
});
document.getElementById('cancel').addEventListener('click', () => send({ action: 'cancel' }));
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
window.__editor = { graph, render, askDelete, setTab: (k) => { tab = k; render(); } };
