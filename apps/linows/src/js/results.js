const BADGE_LABELS = {
  app: 'APP',
  file: 'FILE',
  folder: 'DIR',
  setting: 'SYS',
};

let currentResults = [];
let selectedIndex = -1;
let container = null;

export function init(containerEl) {
  container = containerEl;
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
  select(Math.min(selectedIndex + 1, currentResults.length - 1));
}

export function selectPrev() {
  if (currentResults.length === 0) return;
  select(Math.max(selectedIndex - 1, 0));
}

export function select(index) {
  // Remove previous selection
  const prev = container.querySelector('.result-row.selected');
  if (prev) prev.classList.remove('selected');

  selectedIndex = index;

  const rows = container.querySelectorAll('.result-row');
  if (rows[index]) {
    rows[index].classList.add('selected');
    rows[index].scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }
}

function createRow(result, index) {
  const row = document.createElement('div');
  row.className = 'result-row';
  row.dataset.index = index;

  // Icon (fallback: first letter)
  const icon = document.createElement('div');
  icon.className = 'result-icon';
  icon.textContent = result.title.charAt(0).toUpperCase();
  row.appendChild(icon);

  // Text content
  const text = document.createElement('div');
  text.className = 'result-text';

  const title = document.createElement('div');
  title.className = 'result-title';
  title.textContent = result.title;
  text.appendChild(title);

  const subtitle = document.createElement('div');
  subtitle.className = 'result-path';
  const kindLabels = { app: 'App', file: 'File', folder: 'Folder', setting: 'Setting' };
  subtitle.textContent = kindLabels[result.kind] || result.kind;
  text.appendChild(subtitle);

  row.appendChild(text);

  // Click to select and open
  row.addEventListener('click', () => {
    select(index);
    // Dispatch a custom event so app.js can handle open
    row.dispatchEvent(new CustomEvent('result-activate', { bubbles: true }));
  });

  return row;
}
