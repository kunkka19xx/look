import { getIcon } from '../ipc.js';

const iconCache = new Map();
const pickedMap = new Map(); // key → result

let currentResults = [];
let selectedIndex = -1;
let container = null;
let onSelectionChange = null;
let onPickChange = null;

export function init(containerEl) {
  container = containerEl;
}

export function setOnSelectionChange(callback) {
  onSelectionChange = callback;
}

export function setOnPickChange(callback) {
  onPickChange = callback;
}

export function render(results) {
  currentResults = results;
  container.innerHTML = '';

  if (results.length === 0) {
    container.innerHTML = '<div class="empty-state">No results</div>';
    selectedIndex = -1;
    return;
  }

  results.forEach((result, index) => {
    const row = createRow(result, index);
    container.appendChild(row);
  });

  select(0);
}

export function getSelected() {
  if (selectedIndex >= 0 && selectedIndex < currentResults.length) {
    return currentResults[selectedIndex];
  }
  return null;
}

export function getSelectedIndex() {
  return selectedIndex;
}

export function selectNext() {
  if (currentResults.length === 0) return;
  select((selectedIndex + 1) % currentResults.length);
}

export function selectPrev() {
  if (currentResults.length === 0) return;
  select((selectedIndex - 1 + currentResults.length) % currentResults.length);
}

export function select(index) {
  const prev = container.querySelector('.result-row.selected');
  if (prev) prev.classList.remove('selected');

  selectedIndex = index;

  const rows = container.querySelectorAll('.result-row');
  if (rows[index]) {
    rows[index].classList.add('selected');
    rows[index].scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }

  if (onSelectionChange) {
    onSelectionChange(getSelected());
  }
}

// --- Pick management ---

function pickKey(item) {
  return `${item.kind}|${item.path}`;
}

export function togglePick(item) {
  if (!item) return;
  const key = pickKey(item);
  if (pickedMap.has(key)) {
    pickedMap.delete(key);
  } else {
    pickedMap.set(key, item);
  }
  updatePickedIndicators();
  if (onPickChange) onPickChange(getPickedItems());
}

export function removePick(key) {
  pickedMap.delete(key);
  updatePickedIndicators();
  if (onPickChange) onPickChange(getPickedItems());
}

export function clearPicks() {
  pickedMap.clear();
  updatePickedIndicators();
  if (onPickChange) onPickChange(getPickedItems());
}

export function isPicked(item) {
  return pickedMap.has(pickKey(item));
}

export function getPickedItems() {
  return [...pickedMap.entries()].map(([key, item]) => ({ key, ...item }));
}

export function hasPickedItems() {
  return pickedMap.size > 0;
}

function updatePickedIndicators() {
  const rows = container.querySelectorAll('.result-row');
  rows.forEach((row, i) => {
    const result = currentResults[i];
    if (!result) return;
    const check = row.querySelector('.pick-check');
    if (pickedMap.has(pickKey(result))) {
      if (!check) {
        const el = document.createElement('div');
        el.className = 'pick-check';
        el.innerHTML = '&#10003;';
        row.appendChild(el);
      }
    } else if (check) {
      check.remove();
    }
  });
}

// --- Row creation ---

function createRow(result, index) {
  const row = document.createElement('div');
  row.className = 'result-row';
  row.dataset.index = index;

  // Icon (first letter fallback, async-load real icon)
  const icon = document.createElement('div');
  icon.className = 'result-icon';
  icon.textContent = result.title.charAt(0).toUpperCase();
  row.appendChild(icon);

  loadIcon(icon, result.kind, result.path, result.id);

  // Text content
  const text = document.createElement('div');
  text.className = 'result-text';

  const title = document.createElement('div');
  title.className = 'result-title';
  title.textContent = result.title;
  text.appendChild(title);

  const subtitle = document.createElement('div');
  subtitle.className = 'result-path';
  if (result.subtitle) {
    subtitle.textContent = result.subtitle;
  } else if (result.kind === 'file' || result.kind === 'folder') {
    subtitle.textContent = result.path;
  } else {
    const kindLabels = { app: 'App', setting: 'Setting' };
    subtitle.textContent = kindLabels[result.kind] || result.kind;
  }
  text.appendChild(subtitle);

  row.appendChild(text);

  // Picked indicator
  if (pickedMap.has(pickKey(result))) {
    const el = document.createElement('div');
    el.className = 'pick-check';
    el.innerHTML = '&#10003;';
    row.appendChild(el);
  }

  row.addEventListener('click', () => {
    select(index);
    row.dispatchEvent(new CustomEvent('result-activate', { bubbles: true }));
  });

  return row;
}

function loadIcon(iconEl, kind, path, id) {
  const cacheKey = `${kind}:${path}`;

  if (iconCache.has(cacheKey)) {
    const dataUrl = iconCache.get(cacheKey);
    if (dataUrl) {
      applyIcon(iconEl, dataUrl);
    }
    return;
  }

  getIcon(kind, path, id).then((result) => {
    const dataUrl = result?.data_url || null;
    iconCache.set(cacheKey, dataUrl);
    if (dataUrl) {
      applyIcon(iconEl, dataUrl);
    }
  }).catch(() => {
    iconCache.set(cacheKey, null);
  });
}

function applyIcon(iconEl, dataUrl) {
  const img = document.createElement('img');
  img.src = dataUrl;
  img.alt = '';
  iconEl.textContent = '';
  iconEl.style.background = 'none';
  iconEl.appendChild(img);
}
