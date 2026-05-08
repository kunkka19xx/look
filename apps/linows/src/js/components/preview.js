import { getIcon, getFileMeta, getAppVersion, deleteClipboardEntry } from '../ipc.js';
import { clipboard as clipboardIcon, trash as trashIcon, appIcon, fileIcon, folderIcon, settingIcon } from '../icons.js';

let panel = null;
let currentPath = null;
let onClipDelete = null;

export function init(panelEl) {
  panel = panelEl;
}

export function setOnClipDelete(fn) {
  onClipDelete = fn;
}

export function update(result) {
  if (!result) {
    panel.hidden = true;
    currentPath = null;
    return;
  }

  // Clipboard items use id as cache key (not path, since all share clipboard://history)
  const cacheKey = result.kind === 'clipboard' ? result.id : result.path;
  if (currentPath === cacheKey) return;
  currentPath = cacheKey;

  panel.hidden = false;
  panel.innerHTML = '';

  if (result.kind === 'clipboard') {
    renderClipboardPreview(result);
    return;
  }

  // Header: icon + title + badge + size
  const header = document.createElement('div');
  header.className = 'preview-header';

  const iconWrap = document.createElement('div');
  iconWrap.className = 'preview-icon';
  const isSettings = result.path?.startsWith('settings://') || result.subtitle?.toLowerCase().startsWith('settings');
  const fallbacks = { file: fileIcon, folder: folderIcon, setting: settingIcon };
  iconWrap.innerHTML = isSettings ? settingIcon : (fallbacks[result.kind] || appIcon);
  iconWrap.style.background = 'var(--control-fill)';
  iconWrap.style.color = 'var(--font-secondary)';
  header.appendChild(iconWrap);

  getIcon(result.kind, result.path, result.id).then((res) => {
    if (res?.data_url && currentPath === cacheKey) {
      const img = document.createElement('img');
      img.src = res.data_url;
      img.alt = '';
      iconWrap.innerHTML = '';
      iconWrap.style.background = 'none';
      iconWrap.style.color = '';
      iconWrap.appendChild(img);
    }
  });

  const headerText = document.createElement('div');
  headerText.className = 'preview-header-text';

  const title = document.createElement('div');
  title.className = 'preview-title';
  title.textContent = result.title;
  headerText.appendChild(title);

  const headerSub = document.createElement('div');
  headerSub.className = 'preview-header-sub';

  const badge = document.createElement('span');
  badge.className = `preview-badge kind-${result.kind}`;
  const kindLabels = { app: 'App', file: 'File', folder: 'Folder', setting: 'Setting' };
  badge.textContent = kindLabels[result.kind] || result.kind;
  headerSub.appendChild(badge);

  headerText.appendChild(headerSub);
  header.appendChild(headerText);
  panel.appendChild(header);

  // Metadata rows
  const metaWrap = document.createElement('div');
  metaWrap.className = 'preview-meta';
  panel.appendChild(metaWrap);

  if (result.kind === 'app') {
    renderAppMeta(metaWrap, result, headerSub);
  } else {
    renderFileMeta(metaWrap, result, headerSub);
  }
}

function renderClipboardPreview(result) {
  // Header row: icon + title/date + Delete button
  const header = document.createElement('div');
  header.className = 'preview-header';

  const iconWrap = document.createElement('div');
  iconWrap.className = 'preview-icon';
  iconWrap.innerHTML = clipboardIcon;
  iconWrap.style.background = 'var(--control-fill)';
  iconWrap.style.color = 'var(--font-secondary)';
  header.appendChild(iconWrap);

  const headerText = document.createElement('div');
  headerText.className = 'preview-header-text';

  const title = document.createElement('div');
  title.className = 'preview-title';
  title.textContent = 'Clipboard item';
  headerText.appendChild(title);

  const dateSub = document.createElement('div');
  dateSub.className = 'preview-path';
  dateSub.textContent = `Captured ${result.clipDateMedium}`;
  headerText.appendChild(dateSub);

  header.appendChild(headerText);

  // Delete button
  const delBtn = document.createElement('button');
  delBtn.className = 'preview-clip-delete';
  delBtn.innerHTML = trashIcon + ' Delete';
  delBtn.addEventListener('click', async () => {
    await deleteClipboardEntry(result.clipIndex);
    if (onClipDelete) onClipDelete();
  });
  header.appendChild(delBtn);

  panel.appendChild(header);

  // Badge + counts
  const badgeRow = document.createElement('div');
  badgeRow.className = 'preview-header-sub';
  const badge = document.createElement('span');
  badge.className = 'preview-badge kind-clipboard';
  badge.textContent = 'Clipboard';
  badgeRow.appendChild(badge);
  const counts = document.createElement('span');
  counts.className = 'preview-clip-counts';
  counts.textContent = `${result.clipCharCount} chars  ${result.clipLineCount} lines`;
  badgeRow.appendChild(counts);
  panel.appendChild(badgeRow);

  // Preview label
  const previewLabel = document.createElement('div');
  previewLabel.className = 'preview-clip-label';
  previewLabel.textContent = 'Preview';
  panel.appendChild(previewLabel);

  // Text preview card
  const previewCard = document.createElement('div');
  previewCard.className = 'preview-clip-card';
  const previewText = document.createElement('pre');
  previewText.className = 'preview-clip-text';
  previewText.textContent = result.clipText;
  previewCard.appendChild(previewText);
  panel.appendChild(previewCard);

  // Info rows
  const metaWrap = document.createElement('div');
  metaWrap.className = 'preview-meta';
  metaWrap.appendChild(infoRow('Kind', 'Clipboard'));
  metaWrap.appendChild(infoRow('Captured', result.clipDateMedium));
  panel.appendChild(metaWrap);
}

function renderAppMeta(metaWrap, result, headerSub) {
  // Async version lookup
  getAppVersion(result.path).then((version) => {
    if (currentPath !== result.path) return;
    if (version) {
      // Insert version as first row
      metaWrap.insertBefore(infoRow('Version', version), metaWrap.firstChild);
    }
  });

  metaWrap.appendChild(infoRow('Kind', 'App'));
  metaWrap.appendChild(infoRow('Path', result.path));
}

function renderFileMeta(metaWrap, result, headerSub) {
  getFileMeta(result.path).then((meta) => {
    if (currentPath !== result.path) return;

    if (meta.size != null) {
      const sizeSpan = document.createElement('span');
      sizeSpan.className = 'preview-size';
      sizeSpan.textContent = formatSize(meta.size);
      headerSub.appendChild(sizeSpan);
    }

    if (meta.modified) {
      metaWrap.appendChild(infoRow('Modified', meta.modified));
    }

    metaWrap.appendChild(infoRow('Kind', result.kind === 'folder' ? 'Folder' : 'File'));
    metaWrap.appendChild(infoRow('Path', result.path));

    // Image preview
    if (meta.is_image) {
      const preview = document.createElement('div');
      preview.className = 'preview-image';
      const img = document.createElement('img');
      img.src = convertFileSrc(result.path);
      img.alt = result.title;
      img.onerror = () => preview.remove();
      preview.appendChild(img);
      panel.appendChild(preview);
    }
  });
}

export function clear() {
  if (panel) {
    panel.hidden = true;
    panel.innerHTML = '';
    currentPath = null;
  }
}

function infoRow(label, value) {
  const row = document.createElement('div');
  row.className = 'preview-info-row';

  const l = document.createElement('span');
  l.className = 'preview-info-label';
  l.textContent = label;
  row.appendChild(l);

  const v = document.createElement('span');
  v.className = 'preview-info-value';
  v.textContent = value;
  row.appendChild(v);

  return row;
}

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function convertFileSrc(path) {
  return window.__TAURI__.core.convertFileSrc(path);
}
