import { getIcon } from '../ipc.js';

let panel = null;
let onRemove = null;
let onClear = null;

export function init(panelEl, { onRemoveItem, onClearAll }) {
  panel = panelEl;
  onRemove = onRemoveItem;
  onClear = onClearAll;
}

export function update(pickedItems) {
  if (!pickedItems || pickedItems.length === 0) {
    panel.hidden = true;
    panel.innerHTML = '';
    return;
  }

  panel.hidden = false;
  panel.innerHTML = '';

  // Header
  const header = document.createElement('div');
  header.className = 'picked-header';

  const label = document.createElement('span');
  label.className = 'picked-label';
  label.textContent = `Picked (${pickedItems.length})`;
  header.appendChild(label);

  const clearBtn = document.createElement('button');
  clearBtn.className = 'picked-clear';
  clearBtn.textContent = 'Clear all';
  clearBtn.addEventListener('click', () => {
    if (onClear) onClear();
  });
  header.appendChild(clearBtn);

  panel.appendChild(header);

  // Items list
  const list = document.createElement('div');
  list.className = 'picked-list';

  for (const item of pickedItems) {
    const row = document.createElement('div');
    row.className = 'picked-item';

    const icon = document.createElement('div');
    icon.className = 'picked-icon';
    icon.textContent = item.title.charAt(0).toUpperCase();
    row.appendChild(icon);

    getIcon(item.kind, item.path, item.id).then((res) => {
      if (res?.data_url) {
        const img = document.createElement('img');
        img.src = res.data_url;
        img.alt = '';
        icon.textContent = '';
        icon.style.background = 'none';
        icon.appendChild(img);
      }
    });

    const text = document.createElement('div');
    text.className = 'picked-text';

    const title = document.createElement('div');
    title.className = 'picked-title';
    title.textContent = item.title;
    text.appendChild(title);

    const path = document.createElement('div');
    path.className = 'picked-path';
    path.textContent = item.path;
    text.appendChild(path);

    row.appendChild(text);

    const removeBtn = document.createElement('button');
    removeBtn.className = 'picked-remove';
    removeBtn.innerHTML = '&times;';
    removeBtn.addEventListener('click', () => {
      if (onRemove) onRemove(item.key);
    });
    row.appendChild(removeBtn);

    list.appendChild(row);
  }

  panel.appendChild(list);
}
