// /todo: daily tasks & progress. Port of the macOS TodoCommand/TodoView/
// TodoAnalyticsView trio (apps/macos/.../Views/Commands/Todo*.swift), which
// is the design source of truth. Data lives in the shared look-todo SQLite
// store: load the full set once, edit in memory, write the whole set back on
// Save (Ctrl+S). No autosave.

import { todoList, todoSave } from '../../ipc.js';
import * as banner from '../../components/banner.js';
import {
  listChecks, barChart, flame, plus, save as saveIcon, calendarPlus,
  trash as trashIcon, search as searchIcon, check as checkIcon, activity, calendar, zap,
} from '../../icons.js';

// Mirrors macOS TodoState limits: at most 3 unfinished tasks per day
// (completing one frees a slot) and at most 3 upcoming date groups.
const UNFINISHED_LIMIT = 3;
const FUTURE_GROUP_LIMIT = 3;
// Browse mode shows future + today + this many past days; searching spans
// the full retained year.
const BROWSE_PAST_DAYS = 31;
const NAME_MAX_LEN = 256;
const TREND_DAYS = 30;
const HEATMAP_MAX_WEEKS = 52;
const STREAK_DOTS = 7;
// Heatmap intensity ramp: done-count buckets 0/1/2/3+ map to levels
// 0/1/3/4 of this 5-step accent-opacity scale (matches TodoHeatDay.level).
const HEAT_LEVEL_OPACITY = [0.12, 0.28, 0.5, 0.74, 1];
const HEAT_LEVEL_FOR_DONE = (done) => (done === 0 ? 0 : done === 1 ? 1 : done === 2 ? 3 : 4);
const SAVE_TOAST_SECS = 1.6;

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const HEATMAP_ROW_LABELS = ['', 'M', '', 'W', '', 'F', ''];

// --- State ---
// Map of 'yyyy-MM-dd' -> [{id, name, done, createdAt}]. The date key doubles
// as the task's due_date on save.
let tasksByDay = new Map();
let dirty = false;
let loaded = false;
let visible = false;
let page = 'tasks';
let editingAddKey = null; // day key with an open "Add task" field
let editingTaskRef = null; // {key, id} being renamed
let onQuickChange = null;

// --- DOM refs ---
let panel, searchBar, searchInput, statsBar, toolbar;
let countEl, addDateBtn, saveBtn, daysEl, statsEl;

export function init() {
  panel = document.getElementById('cmd-panel-todo');
  searchBar = document.getElementById('cmd-todo-search-bar');
  searchInput = document.getElementById('cmd-todo-search');
  statsBar = document.getElementById('cmd-todo-stats-bar');
  toolbar = document.getElementById('cmd-todo-toolbar');
  countEl = document.getElementById('cmd-todo-count');
  addDateBtn = document.getElementById('cmd-todo-add-date');
  saveBtn = document.getElementById('cmd-todo-save-btn');
  daysEl = document.getElementById('cmd-todo-days');
  statsEl = document.getElementById('cmd-todo-stats');

  document.getElementById('cmd-todo-search-icon').innerHTML = searchIcon;
  document.getElementById('cmd-todo-stats-icon').innerHTML = barChart;

  searchInput.addEventListener('input', () => renderDays());

  panel.querySelectorAll('.cmd-todo-segmented button').forEach((btn) => {
    btn.addEventListener('click', () => setPage(btn.dataset.page));
  });
  addDateBtn.addEventListener('click', addDate);
  saveBtn.addEventListener('click', persist);

  // All day-card actions are delegated: rows are re-rendered wholesale on
  // every mutation, so per-row listeners would leak.
  daysEl.addEventListener('click', (e) => {
    const el = e.target.closest('[data-act]');
    if (!el) return;
    const { act, group, task } = el.dataset;
    switch (act) {
      case 'toggle': toggleTask(group, task); break;
      case 'del': removeTask(group, task); break;
      case 'complete-all': completeAll(group); break;
      case 'clear-all': clearAll(group); break;
      case 'add-open':
        editingAddKey = group;
        editingTaskRef = null;
        renderDays();
        break;
    }
  });
  daysEl.addEventListener('dblclick', (e) => {
    const nameEl = e.target.closest('.cmd-todo-task-name');
    if (!nameEl) return;
    const row = nameEl.closest('[data-task-row]');
    const key = row.dataset.group;
    if (key < todayKey()) return; // past days are read-only
    editingTaskRef = { key, id: row.dataset.taskRow };
    editingAddKey = null;
    renderDays();
  });
  // Click-away from an open field cancels it (Enter commits, Esc cancels).
  daysEl.addEventListener('focusout', (e) => {
    if (e.target.dataset?.todoField) {
      // Let a click on "add" commit via keydown first; defer the check.
      setTimeout(() => {
        const ae = document.activeElement;
        if (!ae || !ae.dataset?.todoField) closeFields();
      }, 0);
    }
  });

  load();
}

export function enter() {
  visible = true;
  panel.hidden = false;
  ensureTodayGroup();
  setPage('tasks');
  renderAll();
  searchInput.focus();
}

export function exit() {
  visible = false;
  panel.hidden = true;
  editingAddKey = null;
  editingTaskRef = null;
}

export function handleKey(e) {
  // Ctrl+N flips Tasks/Stats, Ctrl+S saves; same pair as macOS Cmd+N/Cmd+S.
  if (e.ctrlKey && !e.shiftKey && !e.altKey && (e.key === 'n' || e.key === 'N')) {
    e.preventDefault();
    setPage(page === 'tasks' ? 'stats' : 'tasks');
    return true;
  }
  if (e.ctrlKey && !e.shiftKey && !e.altKey && (e.key === 's' || e.key === 'S')) {
    e.preventDefault();
    persist();
    return true;
  }

  const ae = document.activeElement;
  if (ae && ae.dataset && ae.dataset.todoField) {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitField(ae);
      return true;
    }
    if (e.key === 'Escape') {
      closeFields();
      searchInput.focus();
      return true; // handled: don't exit command mode
    }
    return false; // typing flows into the field
  }
  return false;
}

// --- Quick view (hint bar "Todo X/Y" on the home screen) ---

export function setOnQuickChange(fn) {
  onQuickChange = fn;
  if (loaded) fireQuickChange();
}

// Reload from the store when nothing would be lost; used on window-show so
// the quick view survives day rollovers and edits from other instances.
export function reloadIfClean() {
  if (!dirty && !visible) load();
}

function fireQuickChange() {
  if (!onQuickChange) return;
  const list = tasksByDay.get(todayKey()) || [];
  onQuickChange({
    done: list.filter((t) => t.done).length,
    total: list.length,
    open: list.filter((t) => !t.done).map((t) => t.name),
  });
}

// --- Persistence ---

async function load() {
  try {
    const rows = await todoList();
    if (dirty) return; // don't clobber edits made while the read was in flight
    tasksByDay = new Map();
    for (const r of rows) {
      if (!tasksByDay.has(r.due_date)) tasksByDay.set(r.due_date, []);
      tasksByDay.get(r.due_date).push({
        id: r.id, name: r.name, done: r.done, createdAt: r.created_at_unix_s,
      });
    }
    loaded = true;
    dirty = false;
  } catch (err) {
    console.error('[todo] load failed:', err);
  }
  fireQuickChange();
  if (visible) renderAll();
}

async function persist() {
  const tasks = [];
  for (const [key, list] of tasksByDay) {
    for (const t of list) {
      tasks.push({
        id: t.id, name: t.name, done: t.done,
        due_date: key, created_at_unix_s: t.createdAt,
      });
    }
  }
  try {
    await todoSave(tasks);
    dirty = false;
    banner.show('Saved', 'success', SAVE_TOAST_SECS);
    renderToolbar();
  } catch (err) {
    banner.show(`Save failed: ${err}`, 'error', 2.0);
  }
}

// --- Date helpers (local time, ISO yyyy-MM-dd keys, the same Gregorian
// keys the macOS app writes, so both clients group identically) ---

const pad2 = (n) => String(n).padStart(2, '0');
const keyOf = (d) => `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;

function dateOf(key) {
  const [y, m, d] = key.split('-').map(Number);
  return new Date(y, m - 1, d);
}

function today() {
  const n = new Date();
  return new Date(n.getFullYear(), n.getMonth(), n.getDate());
}

const todayKey = () => keyOf(today());

function addDays(d, n) {
  const r = new Date(d);
  r.setDate(r.getDate() + n);
  return r;
}

// Both args are local midnights; rounding absorbs DST offsets.
const dayDiff = (from, to) => Math.round((to - from) / 86_400_000);
const monthDay = (d) => `${MONTHS[d.getMonth()]} ${d.getDate()}`;

// Relative phrase for a day, ±6 days like macOS TodoGroup.relativePhrase.
function relativePhrase(diff) {
  if (diff === 0) return 'Today';
  if (diff === 1) return 'Tomorrow';
  if (diff === -1) return 'Yesterday';
  if (diff >= 2 && diff <= 6) return `In ${diff} days`;
  return '';
}

const newTaskId = () => `n${Math.random().toString(36).slice(2, 8)}`;
const nowUnixS = () => Math.floor(Date.now() / 1000);

// --- Mutations (in-memory; Save persists) ---

function markDirty() {
  dirty = true;
  fireQuickChange();
  renderAll();
}

function ensureTodayGroup() {
  if (!tasksByDay.has(todayKey())) tasksByDay.set(todayKey(), []);
}

const openCount = (key) => (tasksByDay.get(key) || []).filter((t) => !t.done).length;

function addTask(key, rawName) {
  const name = rawName.trim().slice(0, NAME_MAX_LEN);
  if (!name || openCount(key) >= UNFINISHED_LIMIT) return false;
  if (!tasksByDay.has(key)) tasksByDay.set(key, []);
  tasksByDay.get(key).push({ id: newTaskId(), name, done: false, createdAt: nowUnixS() });
  markDirty();
  return true;
}

function toggleTask(key, id) {
  if (key < todayKey()) return; // past days are read-only
  const t = (tasksByDay.get(key) || []).find((t) => t.id === id);
  if (!t) return;
  t.done = !t.done;
  markDirty();
}

function editTask(key, id, rawName) {
  const name = rawName.trim().slice(0, NAME_MAX_LEN);
  const t = (tasksByDay.get(key) || []).find((t) => t.id === id);
  if (!t || !name || t.name === name) return;
  t.name = name;
  markDirty();
}

function removeTask(key, id) {
  const list = tasksByDay.get(key);
  if (!list) return;
  const idx = list.findIndex((t) => t.id === id);
  if (idx < 0) return;
  list.splice(idx, 1);
  // A past day emptied out has no add row, so drop the dead card.
  if (list.length === 0 && key < todayKey()) tasksByDay.delete(key);
  markDirty();
}

function completeAll(key) {
  const list = tasksByDay.get(key) || [];
  if (!list.some((t) => !t.done)) return;
  list.forEach((t) => { t.done = true; });
  markDirty();
}

function clearAll(key) {
  if (!(tasksByDay.get(key) || []).length) return;
  tasksByDay.set(key, []);
  markDirty();
}

const futureKeys = () => [...tasksByDay.keys()].filter((k) => k > todayKey());

// Adds the next free future day (tomorrow, then the day after, …).
function addDate() {
  if (futureKeys().length >= FUTURE_GROUP_LIMIT) return;
  let d = addDays(today(), 1);
  while (tasksByDay.has(keyOf(d))) d = addDays(d, 1);
  tasksByDay.set(keyOf(d), []);
  editingAddKey = keyOf(d);
  renderAll();
}

// --- Inline fields (add task / rename) ---

function commitField(input) {
  if (input.dataset.todoField === 'add') {
    if (addTask(input.dataset.group, input.value)) {
      // Field stays open for rapid entry (renderAll re-focuses it) unless
      // the day just hit the unfinished cap.
      if (openCount(input.dataset.group) >= UNFINISHED_LIMIT) editingAddKey = null;
      renderDays();
    }
  } else {
    editTask(input.dataset.group, input.dataset.task, input.value);
    editingTaskRef = null;
    renderDays();
  }
}

function closeFields() {
  if (editingAddKey === null && editingTaskRef === null) return;
  editingAddKey = null;
  editingTaskRef = null;
  renderDays();
}

// --- Search: case/diacritic-insensitive subsequence match over task names
// and date strings ("jul3" finds Jul 3, "di" finds "đi"), whitespace in the
// query ignored. Mirrors macOS TodoSearch. ---

function normalize(text) {
  return text.toLowerCase().normalize('NFKD').replace(/\p{M}/gu, '').replace(/đ/g, 'd');
}

function subseqMatch(needle, target) {
  if (!needle) return true;
  const hay = normalize(target);
  let i = 0;
  for (const ch of hay) {
    if (ch === needle[i]) i += 1;
    if (i === needle.length) return true;
  }
  return false;
}

const dateSearchText = (date, diff) =>
  `${WEEKDAYS[date.getDay()]} ${monthDay(date)} ${relativePhrase(diff)}`;

// --- Rendering ---

function setPage(p) {
  page = p;
  searchBar.hidden = p !== 'tasks';
  toolbar.hidden = p !== 'tasks';
  daysEl.hidden = p !== 'tasks';
  statsBar.hidden = p !== 'stats';
  statsEl.hidden = p !== 'stats';
  panel.querySelectorAll('.cmd-todo-segmented button').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.page === p);
  });
  if (p === 'stats') renderStats();
  else searchInput.focus();
}

function renderAll() {
  if (!visible) return;
  renderToolbar();
  renderDays();
  if (page === 'stats') renderStats();
}

function renderToolbar() {
  if (!visible) return;
  const list = tasksByDay.get(todayKey()) || [];
  const done = list.filter((t) => t.done).length;
  countEl.innerHTML = `${listChecks} <span>${done}/${list.length} done today</span>`;

  const left = FUTURE_GROUP_LIMIT - futureKeys().length;
  addDateBtn.innerHTML = `${calendarPlus} <span>Add date + ${left}</span>`;
  addDateBtn.disabled = left <= 0;
  addDateBtn.title = left > 0 ? `${left} upcoming group(s) left` : `Max ${FUTURE_GROUP_LIMIT} upcoming groups`;

  saveBtn.innerHTML = `${saveIcon} <span>Save</span>${dirty ? '<span class="cmd-todo-dirty-dot"></span>' : ''}`;
  saveBtn.classList.toggle('cmd-todo-btn-dirty', dirty);
  saveBtn.title = 'Save (Ctrl+S)';
}

const escapeHtml = (s) => s.replace(/[&<>"']/g, (c) => (
  { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]
));

function renderDays() {
  if (!visible) return;
  ensureTodayGroup();
  const t = today();
  const needle = normalize(searchInput.value).replace(/\s+/g, '');
  const searching = needle.length > 0;

  let hiddenOlder = 0;
  const cards = [];
  const keys = [...tasksByDay.keys()].sort().reverse();
  for (const key of keys) {
    const date = dateOf(key);
    const diff = dayDiff(t, date);
    if (!searching && diff < -BROWSE_PAST_DAYS) {
      hiddenOlder += 1;
      continue;
    }
    let tasks = tasksByDay.get(key);
    if (searching && !subseqMatch(needle, dateSearchText(date, diff))) {
      tasks = tasks.filter((task) => subseqMatch(needle, task.name));
      if (tasks.length === 0) continue;
    }
    cards.push(dayCardHtml(key, date, diff, tasks));
  }

  let html = cards.join('');
  if (cards.length === 0) {
    html = `<div class="cmd-todo-empty">${searching ? 'No matching tasks or dates' : 'No tasks yet'}</div>`;
  }
  if (hiddenOlder > 0) {
    html += `<div class="cmd-todo-hidden-note">${hiddenOlder} older day${hiddenOlder === 1 ? '' : 's'} hidden · search to view</div>`;
  }
  daysEl.innerHTML = html;

  const field = daysEl.querySelector('[data-todo-field]');
  if (field) {
    field.focus();
    field.setSelectionRange(field.value.length, field.value.length);
  }
}

function ringSvg(done, total) {
  const r = 7;
  const c = 2 * Math.PI * r;
  const f = total > 0 ? done / total : 0;
  return `<svg class="cmd-todo-ring" width="18" height="18" viewBox="0 0 18 18">
    <circle cx="9" cy="9" r="${r}" fill="none" stroke="var(--divider-color)" stroke-width="2.5"/>
    <circle cx="9" cy="9" r="${r}" fill="none" stroke="var(--accent-color)" stroke-width="2.5"
      stroke-linecap="round" stroke-dasharray="${(c * f).toFixed(2)} ${c.toFixed(2)}" transform="rotate(-90 9 9)"/>
  </svg>`;
}

function dayCardHtml(key, date, diff, tasks) {
  const isPast = diff < 0;
  const isToday = diff === 0;
  const phrase = relativePhrase(diff);
  // Ring shows the whole day's progress even when search filters the rows.
  const full = tasksByDay.get(key);
  const done = full.filter((t) => t.done).length;
  // "Today Jul 6" / "Tomorrow Jul 7" / "Sun Jul 5 · Yesterday"
  const usePhraseAsTitle = diff === 0 || diff === 1;
  const title = usePhraseAsTitle ? phrase : WEEKDAYS[date.getDay()];
  const suffix = !usePhraseAsTitle && phrase ? `<span class="cmd-todo-day-rel">· ${phrase}</span>` : '';

  const headerBtns = isPast ? '' : `
    <button class="cmd-todo-icon-btn" data-act="complete-all" data-group="${key}" title="Complete all">${listChecks}</button>
    <button class="cmd-todo-icon-btn" data-act="clear-all" data-group="${key}" title="Clear all tasks">${trashIcon}</button>`;

  const rows = tasks.map((task) => taskRowHtml(key, task, isPast)).join('');

  return `<div class="cmd-todo-day${isPast ? ' cmd-todo-day-past' : ''}">
    <div class="cmd-todo-day-header">
      ${ringSvg(done, full.length)}
      <span class="cmd-todo-day-title${isToday ? ' cmd-todo-day-today' : ''}">${title}</span>
      <span class="cmd-todo-day-date">${monthDay(date)}</span>
      ${suffix}
      <span class="cmd-todo-day-spacer"></span>
      ${headerBtns}
    </div>
    ${rows}
    ${isPast ? '' : addRowHtml(key)}
  </div>`;
}

function taskRowHtml(key, task, isPast) {
  const overdue = isPast && !task.done;
  const editing = editingTaskRef && editingTaskRef.key === key && editingTaskRef.id === task.id;
  const nameHtml = editing
    ? `<input class="cmd-todo-field" data-todo-field="edit" data-group="${key}" data-task="${task.id}"
        value="${escapeHtml(task.name)}" spellcheck="false" autocomplete="off" />`
    : `<span class="cmd-todo-task-name${task.done ? ' cmd-todo-task-done' : ''}"
        ${isPast ? '' : 'title="Double-click to edit"'}>${escapeHtml(task.name)}</span>`;
  return `<div class="cmd-todo-task" data-task-row="${task.id}" data-group="${key}">
    <button class="cmd-todo-checkbox${task.done ? ' checked' : ''}" data-act="toggle"
      data-group="${key}" data-task="${task.id}"
      ${isPast ? 'disabled title="Past days are read-only"' : ''}>${task.done ? checkIcon : ''}</button>
    ${nameHtml}
    ${overdue ? '<span class="cmd-todo-overdue">OVERDUE</span>' : ''}
    <button class="cmd-todo-task-del" data-act="del" data-group="${key}" data-task="${task.id}" title="Remove">×</button>
  </div>`;
}

function addRowHtml(key) {
  if (editingAddKey === key) {
    return `<div class="cmd-todo-add cmd-todo-add-active">
      ${plus}
      <input class="cmd-todo-field" data-todo-field="add" data-group="${key}"
        placeholder="Task name, then ↵" spellcheck="false" autocomplete="off" />
      <span class="cmd-todo-add-hint">↵ add · esc</span>
    </div>`;
  }
  if (openCount(key) >= UNFINISHED_LIMIT) {
    return `<div class="cmd-todo-add cmd-todo-add-limit">
      ${UNFINISHED_LIMIT} unfinished · complete one to add more
    </div>`;
  }
  return `<button class="cmd-todo-add" data-act="add-open" data-group="${key}">${plus} Add task</button>`;
}

// --- Stats page (all math mirrors macOS TodoAnalytics) ---

function dayCounts() {
  const counts = new Map();
  for (const [key, list] of tasksByDay) {
    counts.set(key, {
      done: list.filter((t) => t.done).length,
      total: list.length,
    });
  }
  return counts;
}

// done/total across the current Sunday-based week or calendar month.
function periodStat(counts, period) {
  const t = today();
  const weekStart = addDays(t, -t.getDay());
  let done = 0;
  let total = 0;
  for (const [key, c] of counts) {
    const d = dateOf(key);
    const inPeriod = period === 'week'
      ? d >= weekStart && d <= addDays(weekStart, 6)
      : d.getFullYear() === t.getFullYear() && d.getMonth() === t.getMonth();
    if (inPeriod) {
      done += c.done;
      total += c.total;
    }
  }
  return { done, total };
}

// Consecutive days with >=1 completed task, counting back from today.
// A zero-done today doesn't break the streak, it just doesn't extend it.
function streakDays(counts) {
  const doneOn = (d) => (counts.get(keyOf(d))?.done ?? 0) > 0;
  let streak = doneOn(today()) ? 1 : 0;
  let cursor = today();
  while (doneOn(addDays(cursor, -1))) {
    streak += 1;
    cursor = addDays(cursor, -1);
  }
  return streak;
}

// Completed-task count per day for the trailing 30 days, oldest first.
function trendData(counts) {
  const t = today();
  const arr = [];
  for (let offset = -(TREND_DAYS - 1); offset <= 0; offset += 1) {
    arr.push(counts.get(keyOf(addDays(t, offset)))?.done ?? 0);
  }
  return arr;
}

function donutHtml(label, stat) {
  const f = stat.total > 0 ? stat.done / stat.total : 0;
  const pct = Math.round(f * 100);
  const r = 26;
  const c = 2 * Math.PI * r;
  return `<div class="cmd-todo-stat-cell">
    <div class="cmd-todo-stat-label">${label} <b>${pct}%</b></div>
    <svg width="68" height="68" viewBox="0 0 68 68">
      <circle cx="34" cy="34" r="${r}" fill="none" stroke="var(--divider-color)" stroke-width="5"/>
      <circle cx="34" cy="34" r="${r}" fill="none" stroke="var(--accent-color)" stroke-width="5"
        stroke-linecap="round" stroke-dasharray="${(c * f).toFixed(2)} ${c.toFixed(2)}" transform="rotate(-90 34 34)"/>
      <text x="34" y="32" text-anchor="middle" class="cmd-todo-donut-done">${stat.done}</text>
      <text x="34" y="46" text-anchor="middle" class="cmd-todo-donut-total">${stat.total}</text>
    </svg>
  </div>`;
}

function streakHtml(streak, counts) {
  const t = today();
  let dots = '';
  for (let offset = -(STREAK_DOTS - 1); offset <= 0; offset += 1) {
    const active = (counts.get(keyOf(addDays(t, offset)))?.done ?? 0) > 0;
    dots += `<span class="cmd-todo-streak-dot${active ? ' active' : ''}"></span>`;
  }
  return `<div class="cmd-todo-stat-cell">
    <div class="cmd-todo-stat-label">STREAK</div>
    <div class="cmd-todo-streak">
      <span class="cmd-todo-streak-flame">${flame}</span>
      <span class="cmd-todo-streak-days"><b>${streak}</b> day${streak === 1 ? '' : 's'}</span>
    </div>
    <div class="cmd-todo-streak-dots">${dots}</div>
  </div>`;
}

function trendSvg(data, width) {
  const h = 96;
  const padX = 8;
  const padY = 10;
  const maxV = Math.max(UNFINISHED_LIMIT, ...data);
  const stepX = (width - padX * 2) / (data.length - 1);
  const x = (i) => (padX + i * stepX).toFixed(1);
  const y = (v) => (h - padY - (v / maxV) * (h - padY * 2)).toFixed(1);

  const points = data.map((v, i) => `${x(i)},${y(v)}`).join(' ');
  const dots = data.map((v, i) => {
    const isLast = i === data.length - 1;
    return `<circle cx="${x(i)}" cy="${y(v)}" r="${isLast ? 3.5 : 2}"
      fill="${isLast || v > 0 ? 'var(--accent-color)' : 'var(--panel-fill)'}"
      stroke="var(--accent-color)" stroke-width="1"/>`;
  }).join('');

  return `<svg width="${width}" height="${h}" viewBox="0 0 ${width} ${h}">
    <line x1="${padX}" y1="${y(0)}" x2="${width - padX}" y2="${y(0)}" stroke="var(--divider-color)" stroke-width="1"/>
    <polyline points="${points}" fill="none" stroke="var(--accent-color)" stroke-width="1.5"/>
    ${dots}
  </svg>`;
}

function heatmapHtml(counts, width) {
  const cellStep = 13; // 10px cell + 3px gap
  const labelW = 18;
  const weeks = Math.max(1, Math.min(HEATMAP_MAX_WEEKS, Math.floor((width - labelW) / cellStep)));
  const t = today();
  const lastSunday = addDays(t, -t.getDay());

  const labels = HEATMAP_ROW_LABELS
    .map((l) => `<span class="cmd-todo-heat-label">${l}</span>`)
    .join('');

  let cols = '';
  for (let w = weeks - 1; w >= 0; w -= 1) {
    const colSunday = addDays(lastSunday, -7 * w);
    let cells = '';
    for (let d = 0; d < 7; d += 1) {
      const date = addDays(colSunday, d);
      if (date > t) {
        cells += '<span class="cmd-todo-heat-cell" style="opacity:0"></span>';
        continue;
      }
      const c = counts.get(keyOf(date)) || { done: 0, total: 0 };
      const opacity = HEAT_LEVEL_OPACITY[HEAT_LEVEL_FOR_DONE(c.done)];
      const tip = `${WEEKDAYS[date.getDay()]}, ${monthDay(date)}: ${c.total > 0 ? `${c.done}/${c.total} done` : 'no tasks'}`;
      cells += `<span class="cmd-todo-heat-cell" style="opacity:${opacity}" title="${tip}"></span>`;
    }
    cols += `<div class="cmd-todo-heat-col">${cells}</div>`;
  }

  const legend = HEAT_LEVEL_OPACITY
    .map((o) => `<span class="cmd-todo-heat-cell" style="opacity:${o}"></span>`)
    .join('');

  return `<div class="cmd-todo-heatmap">
    <div class="cmd-todo-heat-col cmd-todo-heat-labels">${labels}</div>
    ${cols}
  </div>
  <div class="cmd-todo-heat-legend">Less ${legend} More</div>`;
}

function insightsHtml(trend) {
  const total = trend.reduce((a, b) => a + b, 0);
  const tiles = [
    [(total / trend.length).toFixed(1), 'AVG / DAY'],
    [String(Math.max(...trend)), 'BEST DAY'],
    [`${trend.filter((v) => v > 0).length}/${trend.length}`, 'ACTIVE DAYS'],
    [String(total), `DONE · ${trend.length}D`],
  ];
  return tiles.map(([value, label]) => `<div class="cmd-todo-insight">
    <div class="cmd-todo-insight-value">${value}</div>
    <div class="cmd-todo-insight-label">${label}</div>
  </div>`).join('<div class="cmd-todo-insight-sep"></div>');
}

function renderStats() {
  const counts = dayCounts();
  const trend = trendData(counts);
  const t = today();
  // Panel is visible when this runs (setPage/renderAll guard), so widths
  // are real. Fall back defensively for the first paint.
  const width = Math.max(280, (statsEl.clientWidth || 560) - 36);

  statsEl.innerHTML = `
    <div class="cmd-todo-card cmd-todo-stat-strip">
      ${donutHtml('THIS WEEK', periodStat(counts, 'week'))}
      <div class="cmd-todo-stat-sep"></div>
      ${donutHtml('THIS MONTH', periodStat(counts, 'month'))}
      <div class="cmd-todo-stat-sep"></div>
      ${streakHtml(streakDays(counts), counts)}
    </div>

    <div class="cmd-todo-section-title">${activity} COMPLETION TREND · ${TREND_DAYS} DAYS</div>
    <div class="cmd-todo-card">
      ${trendSvg(trend, width)}
      <div class="cmd-todo-trend-axis">
        <span>${monthDay(addDays(t, -(TREND_DAYS - 1)))}</span>
        <span>${monthDay(addDays(t, -Math.floor(TREND_DAYS / 2)))}</span>
        <span>${monthDay(t)}</span>
      </div>
    </div>

    <div class="cmd-todo-section-title">${calendar} ACTIVITY · LAST YEAR</div>
    <div class="cmd-todo-card">${heatmapHtml(counts, width)}</div>

    <div class="cmd-todo-section-title">${zap} INSIGHTS · LAST ${TREND_DAYS} DAYS (TASKS)</div>
    <div class="cmd-todo-card cmd-todo-insights">${insightsHtml(trend)}</div>`;
}
