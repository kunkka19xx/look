import { getIcon, getFileMeta, getAppVersion, deleteClipboardEntry, highlightFile, listFolder, openPath } from '../ipc.js';
import { clipboard as clipboardIcon, trash as trashIcon, appIcon, fileIcon, folderIcon, settingIcon } from '../icons.js';

let panel = null;
let currentPath = null;
let onClipDelete = null;
let highlightTimer = null;

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

  if (highlightTimer) { clearTimeout(highlightTimer); highlightTimer = null; }
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

  // Preview placeholder — sits between header and metadata (matches macOS order)
  const previewSlot = document.createElement('div');
  previewSlot.className = 'preview-slot';
  panel.appendChild(previewSlot);

  // Metadata rows
  const metaWrap = document.createElement('div');
  metaWrap.className = 'preview-meta';
  panel.appendChild(metaWrap);

  if (result.kind === 'app') {
    renderAppMeta(metaWrap, result, headerSub);
  } else {
    renderFileMeta(metaWrap, previewSlot, result, headerSub);
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

function renderFileMeta(metaWrap, previewSlot, result, headerSub) {
  const cacheKey = result.path;

  // Metadata: size (in header), then Kind → Path → Modified (matches macOS order)
  getFileMeta(result.path).then((meta) => {
    if (currentPath !== cacheKey) return;

    if (meta.size != null) {
      const sizeSpan = document.createElement('span');
      sizeSpan.className = 'preview-size';
      sizeSpan.textContent = formatSize(meta.size);
      headerSub.appendChild(sizeSpan);
    }

    metaWrap.appendChild(infoRow('Kind', result.kind === 'folder' ? 'Folder' : 'File'));
    metaWrap.appendChild(infoRow('Path', result.path));

    if (meta.modified) {
      metaWrap.appendChild(infoRow('Modified', meta.modified));
    }

    // Image preview — inserted into previewSlot (between header and metadata)
    if (meta.is_image) {
      const preview = document.createElement('div');
      preview.className = 'preview-image';
      const img = document.createElement('img');
      img.src = convertFileSrc(result.path);
      img.alt = result.title;
      img.onerror = () => preview.remove();
      preview.appendChild(img);
      previewSlot.appendChild(preview);
    }
  });

  // Text/code file preview with syntax highlighting.
  // 150ms debounce so rapid arrow-key navigation skips intermediate files
  // (matches macOS TextFilePreview dwell behavior).
  // Inserted into previewSlot (between header and metadata).
  if (result.kind === 'file') {
    if (highlightTimer) clearTimeout(highlightTimer);
    highlightTimer = setTimeout(() => {
      if (currentPath !== cacheKey) return;
      highlightFile(result.path).then((res) => {
        if (!res || currentPath !== cacheKey) return;
        const codeWrap = document.createElement('div');
        codeWrap.className = 'preview-code';
        const pre = document.createElement('pre');
        pre.className = 'preview-code-text';
        pre.innerHTML = res.html;
        codeWrap.appendChild(pre);
        if (res.truncated) {
          const hint = document.createElement('div');
          hint.className = 'preview-code-truncated';
          hint.textContent = 'File truncated at 64 KB';
          codeWrap.appendChild(hint);
        }
        previewSlot.appendChild(codeWrap);
      });
    }, 150);
  }

  // Folder content listing — flat list with counts, clickable items.
  if (result.kind === 'folder') {
    listFolder(result.path).then((listing) => {
      if (!listing || currentPath !== cacheKey) return;

      // Consolidate item count into header badge area (#6)
      const total = listing.folder_count + listing.file_count;
      const countParts = [];
      if (listing.folder_count > 0) countParts.push(`${listing.folder_count} folder${listing.folder_count !== 1 ? 's' : ''}`);
      if (listing.file_count > 0) countParts.push(`${listing.file_count} file${listing.file_count !== 1 ? 's' : ''}`);
      if (countParts.length > 0) {
        const countSpan = document.createElement('span');
        countSpan.className = 'preview-size';
        countSpan.textContent = countParts.join(', ');
        headerSub.appendChild(countSpan);
      }

      const wrap = document.createElement('div');
      wrap.className = 'preview-folder';

      // Empty folder state (#8)
      if (total === 0) {
        const empty = document.createElement('div');
        empty.className = 'preview-folder-empty';
        empty.textContent = 'Empty folder';
        wrap.appendChild(empty);
        previewSlot.appendChild(wrap);
        return;
      }

      // Item list
      const list = document.createElement('div');
      list.className = 'preview-folder-list';
      if (listing.truncated) list.classList.add('is-truncated');

      const pathSep = result.path.includes('\\') ? '\\' : '/';
      let foldersDone = false;
      for (const item of listing.items) {
        // Separator between folders and files (#1)
        if (!item.is_dir && !foldersDone && listing.folder_count > 0) {
          foldersDone = true;
          const sep = document.createElement('div');
          sep.className = 'preview-folder-separator';
          list.appendChild(sep);
        }

        const row = document.createElement('div');
        row.className = 'preview-folder-item';
        row.setAttribute('role', 'button');
        row.tabIndex = -1;

        const icon = document.createElement('span');
        icon.className = 'preview-folder-item-icon';
        // File extension color hints (#2)
        if (!item.is_dir) {
          const ext = item.name.includes('.') ? item.name.split('.').pop().toLowerCase() : '';
          // Only add class for safe extensions (alphanumeric) — classList.add
          // throws InvalidCharacterError on names with spaces or special chars
          if (ext && /^[a-z0-9]+$/.test(ext)) icon.classList.add(`ext-${ext}`);
        }
        icon.innerHTML = item.is_dir ? folderIcon : fileIcon;
        row.appendChild(icon);

        const name = document.createElement('span');
        name.className = 'preview-folder-item-name';
        name.textContent = item.name;
        name.title = item.name;
        row.appendChild(name);

        // Inline file size (#5)
        if (!item.is_dir && item.size != null) {
          const size = document.createElement('span');
          size.className = 'preview-folder-item-size';
          size.textContent = formatSize(item.size);
          row.appendChild(size);
        }

        const itemPath = result.path + pathSep + item.name;
        const itemKind = item.is_dir ? 'folder' : 'file';
        row.addEventListener('click', () => openPath(itemPath, itemKind, ''));

        list.appendChild(row);
      }
      wrap.appendChild(list);

      previewSlot.appendChild(wrap);
    });
  }
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
