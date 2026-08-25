import {
    getIcon,
    getFileMeta,
    getAppVersion,
    deleteClipboardEntry,
    highlightFile,
    highlightShell,
    sourcePreview,
    copyToClipboard,
    listFolder,
    openPath,
    processDetail,
    processCpu,
} from '../ipc.js';
import {
    clipboard as clipboardIcon,
    trash as trashIcon,
    cpu as cpuIcon,
    appIcon,
    fileIcon,
    folderIcon,
    settingIcon,
    globeLg,
    calculatorLg,
    copy as copyIcon,
} from '../icons.js';
import * as sourceblocks from './sourceblocks.js';
import * as banner from './banner.js';
import { classifyResultId, WEB_URL_OPEN_SUBTITLE } from '../catalog.js';
import * as qactions from './qactions.js';
import * as actionmenu from './actionmenu.js';
import { canRunElevated } from '../platform.js';

let panel = null;
let currentPath = null;
let onClipDelete = null;
let highlightTimer = null;
// PID of the process currently previewed, so the on-demand CPU measure (ps"
// Enter) targets the right process and ignores a stale request after the
// selection moved on.
let currentProcPid = null;

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
        qactions.clear();
        actionmenu.close();
        return;
    }

    // Clipboard items use id as cache key (not path, since all share
    // clipboard://history); so do block rows, which may name no path at all and
    // would otherwise all share the empty one. A block row keys on `rowKey`
    // rather than its id alone: what the panel shows is expanded against the
    // levels above it as well, so the same id reached at two depths is two
    // panels.
    const cacheKey = sourceblocks.isSourceRow(result.id)
        ? sourceblocks.rowKey(result)
        : result.kind === 'clipboard' || result.kind === 'action'
          ? result.id
          : result.path;
    if (currentPath === cacheKey) return;
    currentPath = cacheKey;
    // The menu lists the SELECTED row's verbs, so it must not survive a move to
    // another row and offer the previous one's. Re-rendering the same result (a
    // window show) returns above and leaves it alone.
    actionmenu.close();

    if (highlightTimer) {
        clearTimeout(highlightTimer);
        highlightTimer = null;
    }
    panel.hidden = false;
    panel.innerHTML = '';
    panel.classList.remove('is-block', 'has-block-extra');
    currentProcPid = null;
    // The panel was wiped: invalidate any in-flight Quick Actions render so a
    // late response can't append a stale section for the previous result.
    qactions.clear();

    if (result.kind === 'clipboard') {
        renderClipboardPreview(result);
        return;
    }

    if (result.kind === 'process') {
        renderProcessPreview(result);
        return;
    }

    // A block's row with nothing on disk has no file to describe, so the panel
    // answers the only question that matters before Enter: what is about to
    // run. A row that names a path IS that file (`format = "json"`), so it
    // previews like one, with what the block adds appended below.
    if (result.kind === 'action' && sourceblocks.isSourceRow(result.id)) {
        if (result.path) {
            renderFileBackedBlock(result, cacheKey);
        } else {
            renderBlockPreview(result, cacheKey);
        }
        return;
    }

    // Synthetic rows with no file behind them get their own preview layout;
    // must run before the generic app branch below, since a URL row's kind
    // is `app` and its path is a URL, so the file/app metadata path would break.
    const classified = classifyResultId(result.id);
    switch (classified?.kind) {
        case 'webSuggestion':
            // Mirror macOS WebSuggestionPreviewView: a big magnifying-glass
            // icon, the suggestion text, "Search Google", and an Enter hint.
            renderWebSuggestionPreview(classified.text);
            return;
        case 'webUrl':
            // Same layout as the web-suggestion preview but with a globe and
            // the row's subtitle ("Open in browser" / "Recently opened").
            renderWebUrlPreview(classified.url, result.subtitle || WEB_URL_OPEN_SUBTITLE);
            return;
        case 'calc':
            renderCalcPreview(result);
            return;
    }

    renderStandard(result, cacheKey);
}

/** Swaps the placeholder glyph for the real icon, unless the selection moved on. */
function loadPreviewIcon(iconWrap, kind, path, id, cacheKey) {
    getIcon(kind, path, id)
        .then((res) => {
            if (!res?.data_url || currentPath !== cacheKey) return;
            const img = document.createElement('img');
            img.src = res.data_url;
            img.alt = '';
            iconWrap.innerHTML = '';
            iconWrap.style.background = 'none';
            iconWrap.style.color = '';
            iconWrap.appendChild(img);
        })
        // A refused read leaves the placeholder glyph, which is what the wrap
        // already draws.
        .catch((err) => console.warn('preview: could not read an icon', err));
}

/** The ordinary panel: icon, title, kind badge, then the file or app detail. */
function renderStandard(result, cacheKey) {
    // Header: icon + title + badge + size
    const header = document.createElement('div');
    header.className = 'preview-header';

    const iconWrap = document.createElement('div');
    iconWrap.className = 'preview-icon';
    const isSettings =
        result.path?.startsWith('settings://') ||
        result.subtitle?.toLowerCase().startsWith('settings');
    const fallbacks = { file: fileIcon, folder: folderIcon, setting: settingIcon };
    iconWrap.innerHTML = isSettings ? settingIcon : fallbacks[result.kind] || appIcon;
    iconWrap.style.background = 'var(--control-fill)';
    iconWrap.style.color = 'var(--font-secondary)';
    header.appendChild(iconWrap);

    loadPreviewIcon(iconWrap, result.kind, result.path, result.id, cacheKey);

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
    // A row a user's block produced says which block; the kind is on the icon
    // and in the metadata below, and where it came from is what the list cannot
    // otherwise tell you.
    badge.textContent = sourceblocks.blockName(result.id) || kindLabels[result.kind] || result.kind;
    headerSub.appendChild(badge);

    headerText.appendChild(headerSub);
    header.appendChild(headerText);
    panel.appendChild(header);

    // Quick Actions slot - directly beneath the header, above the preview and
    // metadata rows, matching macOS ResultPreviewView (QuickActionsSection
    // renders right after the header). A fixed slot keeps the position stable
    // while the section fills in asynchronously.
    const qactionsSlot = document.createElement('div');
    panel.appendChild(qactionsSlot);

    // Preview placeholder - sits between header and metadata (matches macOS order)
    const previewSlot = document.createElement('div');
    previewSlot.className = 'preview-slot';
    panel.appendChild(previewSlot);

    // Metadata rows
    const metaWrap = document.createElement('div');
    metaWrap.className = 'preview-meta';
    panel.appendChild(metaWrap);

    if (result.kind === 'app') {
        renderAppMeta(metaWrap, result, headerSub, cacheKey);
    } else {
        renderFileMeta(metaWrap, previewSlot, result, headerSub, cacheKey);
    }

    // Quick Actions - interactive controls for results the shared catalog
    // marks actionable (settings toggles). Fills the slot beneath the header;
    // appends nothing for the rest.
    qactions.render(qactionsSlot, result);
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
        try {
            await deleteClipboardEntry(result.clipTimestamp, result.clipText);
            if (onClipDelete) onClipDelete();
        } catch (err) {
            console.error('Delete clipboard entry failed:', err);
        }
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

function renderProcessPreview(result) {
    currentProcPid = result.procPid;
    const cacheKey = result.path;

    // Header: cpu glyph + process name + Process badge and PID
    const header = document.createElement('div');
    header.className = 'preview-header';

    const iconWrap = document.createElement('div');
    iconWrap.className = 'preview-icon';
    iconWrap.innerHTML = cpuIcon;
    iconWrap.style.background = 'var(--control-fill)';
    iconWrap.style.color = 'var(--font-secondary)';
    header.appendChild(iconWrap);

    // App-backed process: swap the generic glyph for the real app icon.
    if (result.iconPath) {
        loadPreviewIcon(iconWrap, 'app', result.iconPath, result.id, cacheKey);
    }

    const headerText = document.createElement('div');
    headerText.className = 'preview-header-text';

    const title = document.createElement('div');
    title.className = 'preview-title';
    title.textContent = result.procName;
    headerText.appendChild(title);

    const sub = document.createElement('div');
    sub.className = 'preview-header-sub';
    const badge = document.createElement('span');
    badge.className = 'preview-badge kind-process';
    badge.textContent = 'Process';
    sub.appendChild(badge);
    const pidSpan = document.createElement('span');
    pidSpan.className = 'preview-size';
    pidSpan.textContent = `PID ${result.procPid}`;
    sub.appendChild(pidSpan);
    headerText.appendChild(sub);

    header.appendChild(headerText);
    panel.appendChild(header);

    // Command line: the disambiguator before a kill (many node/python share a name)
    const cmdLabel = document.createElement('div');
    cmdLabel.className = 'preview-clip-label';
    cmdLabel.textContent = 'Command';
    panel.appendChild(cmdLabel);
    const cmdCard = document.createElement('div');
    cmdCard.className = 'preview-clip-card';
    const cmdText = document.createElement('pre');
    cmdText.className = 'preview-clip-text';
    cmdText.textContent = '…';
    cmdCard.appendChild(cmdText);
    panel.appendChild(cmdCard);

    // Meta rows. CPU stays "Enter to measure" until the on-demand sample runs.
    const metaWrap = document.createElement('div');
    metaWrap.className = 'preview-meta';
    const memRow = infoRow('Memory', '…');
    const userRow = infoRow('User', '…');
    const ppidRow = infoRow('Parent PID', '…');
    const startRow = infoRow('Started', '…');
    const cpuRow = infoRow('CPU', 'Enter to measure');
    cpuRow.querySelector('.preview-info-value').classList.add('preview-proc-cpu-val');
    metaWrap.append(memRow, userRow, ppidRow, startRow, cpuRow);
    // Listening ports, only when the process holds any.
    const ports = result.procPorts || [];
    if (ports.length) {
        metaWrap.appendChild(infoRow('Ports', ports.map((p) => `:${p}`).join('  ')));
    }
    panel.appendChild(metaWrap);

    const setVal = (row, text) => {
        row.querySelector('.preview-info-value').textContent = text;
    };
    // Platforms without detail support and IPC failures (process exited between
    // listing and preview) both degrade to name only.
    const degrade = () => {
        if (currentPath !== cacheKey) return;
        cmdText.textContent = result.procName;
        setVal(memRow, 'unavailable');
        setVal(userRow, 'unavailable');
        setVal(ppidRow, 'unavailable');
        setVal(startRow, 'unavailable');
    };
    processDetail(result.procPid)
        .then((d) => {
            if (currentPath !== cacheKey) return;
            if (!d) {
                degrade();
                return;
            }
            cmdText.textContent = d.cmdline || result.procName;
            setVal(memRow, d.rss_kb > 0 ? formatSize(d.rss_kb * 1024) : 'n/a');
            setVal(userRow, d.user || 'n/a');
            setVal(ppidRow, String(d.ppid));
            setVal(startRow, formatStart(d.start_epoch));
        })
        .catch(degrade);
}

// Sample CPU for the previewed process (bound to ps" Enter). No-op unless a
// process is previewed; drops the result if the selection moved on.
export async function measureCpu() {
    if (currentProcPid == null) return;
    const pid = currentProcPid;
    const valEl = panel.querySelector('.preview-proc-cpu-val');
    if (!valEl) return;
    valEl.textContent = 'measuring…';
    try {
        const pct = await processCpu(pid);
        if (currentProcPid !== pid) return;
        valEl.textContent = pct == null ? 'n/a' : `${pct.toFixed(1)}%`;
    } catch (err) {
        console.error('CPU measure failed:', err);
        if (currentProcPid === pid) valEl.textContent = 'n/a';
    }
}

function formatStart(epoch) {
    if (!epoch) return 'n/a';
    return new Date(epoch * 1000).toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    });
}

function renderAppMeta(metaWrap, result, headerSub, cacheKey) {
    // Async version lookup
    getAppVersion(result.path).then((version) => {
        if (currentPath !== cacheKey) return;
        if (version) {
            // Insert version as first row
            metaWrap.insertBefore(infoRow('Version', version), metaWrap.firstChild);
        }
    });

    metaWrap.appendChild(infoRow('Kind', 'App'));
    metaWrap.appendChild(infoRow('Path', result.path));
    if (canRunElevated(result)) {
        metaWrap.appendChild(infoRow('Run as admin', 'Ctrl+Shift+Enter'));
    }
}

function renderFileMeta(metaWrap, previewSlot, result, headerSub, cacheKey) {
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

        // Image preview - inserted into previewSlot (between header and metadata)
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

    // Folder content listing - flat list with counts, clickable items.
    if (result.kind === 'folder') {
        listFolder(result.path).then((listing) => {
            if (!listing || currentPath !== cacheKey) return;

            // Consolidate item count into header badge area (#6)
            const total = listing.folder_count + listing.file_count;
            const countParts = [];
            if (listing.folder_count > 0)
                countParts.push(
                    `${listing.folder_count} folder${listing.folder_count !== 1 ? 's' : ''}`,
                );
            if (listing.file_count > 0)
                countParts.push(`${listing.file_count} file${listing.file_count !== 1 ? 's' : ''}`);
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
                    const ext = item.name.includes('.')
                        ? item.name.split('.').pop().toLowerCase()
                        : '';
                    // Only add class for safe extensions (alphanumeric) - classList.add
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
        qactions.clear();
        // The menu lives in the panel we just wiped.
        actionmenu.close();
    }
}

// Re-read the current result's Quick Action states in place. Called when the
// window is re-shown: the selection survives hide/show, so a cached state can
// be stale if the system changed while hidden (e.g. Bluetooth flipped).
export function refreshQuickActions() {
    qactions.refresh();
}

// Right half of the clipboard empty state - the "How to use" tips card that
// pairs with the results list's "Clipboard History" info (macOS
// ClipboardEmptyHelpView). Static content, safe as innerHTML.
export function showClipboardHelp() {
    if (!panel) return;
    currentPath = null;
    qactions.clear();
    actionmenu.close();
    panel.hidden = false;
    panel.innerHTML = `
    <div class="preview-clip-help">
      <div class="preview-clip-help-title">How to use</div>
      <div class="preview-clip-help-line">• Type <kbd>c"</kbd> to list latest 10 clips</div>
      <div class="preview-clip-help-line">• Type <kbd>c"mail</kbd> to filter</div>
      <div class="preview-clip-help-line">• Press <kbd>Enter</kbd> to copy selected item</div>
    </div>`;
}

/**
 * A block's row that names a path: the file's own panel, with what the block
 * adds under it. The row IS that file, so everything a file row shows (the
 * thumbnail, the text preview, the folder listing, size and modified) is what
 * the user is looking for; the block only adds what Enter does and where it was
 * declared.
 */
async function renderFileBackedBlock(result, cacheKey) {
    let meta = null;
    try {
        meta = await getFileMeta(result.path);
    } catch (err) {
        console.warn('preview: could not stat a block row', err);
    }
    if (currentPath !== cacheKey) return;

    // The block never says which it is, so the filesystem answers: a folder
    // gets its listing, a file its preview.
    renderStandard({ ...result, kind: meta?.is_dir ? 'folder' : 'file' }, cacheKey);

    // The block's own section is claimed NOW, empty, so it always sits under the
    // file's metadata. Filling it later would otherwise land above or below
    // those rows depending on which read answered first - one file's panel
    // ordered differently from the next one's.
    const section = document.createElement('div');
    section.className = 'preview-meta preview-block-extra';
    // The panel becomes a column so the section sits at its FOOT, wherever the
    // metadata above it ends: the declaration belongs in one place on every
    // row, not wherever the content happens to stop.
    panel.classList.add('has-block-extra');
    panel.appendChild(section);

    const block = await sourceblocks.loadDetail(result);
    if (!block || currentPath !== cacheKey) return;

    if (block.steps.length > 0) {
        const label = document.createElement('div');
        label.className = 'preview-section-label';
        label.textContent = 'Enter runs';
        section.appendChild(label);
        section.appendChild(await stepsBlock(block.steps));
        if (currentPath !== cacheKey) return;
    }
    if (block.file) {
        section.appendChild(infoRow('Declared in', sourceblocks.tildePath(block.file)));
    }

    const preview = await readSourcePreview(result);
    if (!preview || currentPath !== cacheKey) return;
    section.appendChild(blockPreviewBody(preview));
}

/**
 * The panel for a row a user-declared block produced.
 *
 * Two reads, in that order: the declaration is cheap and goes up first, then
 * the declared `preview`, which runs a command. A late answer must not land in
 * the panel of a row the user has already left, so both check the cache key.
 */
async function renderBlockPreview(result, cacheKey) {
    // A column of its own: the declaring file sits at the BOTTOM of the panel
    // (macOS puts a Spacer above it), and only this preview wants that.
    const column = document.createElement('div');
    column.className = 'preview-block';
    panel.classList.add('is-block');
    panel.appendChild(column);

    const header = document.createElement('div');
    header.className = 'preview-header';

    const iconWrap = document.createElement('div');
    iconWrap.className = 'preview-icon';
    iconWrap.innerHTML = sourceblocks.declaredIconHtml(result) || sourceblocks.actionIconHtml;
    iconWrap.style.background = 'var(--control-fill)';
    iconWrap.style.color = 'var(--accent-color)';
    const declaredPath = sourceblocks.declaredIconPath(result);
    if (declaredPath) {
        loadPreviewIcon(iconWrap, 'declared', declaredPath, result.id, cacheKey);
    }
    header.appendChild(iconWrap);

    const headerText = document.createElement('div');
    headerText.className = 'preview-header-text';
    const title = document.createElement('div');
    title.className = 'preview-title';
    title.textContent = result.title;
    headerText.appendChild(title);
    // No kind badge: the subtitle is already the block's name, and the panel
    // saying it twice reads as a bug rather than as emphasis (macOS
    // ResultPreviewView.actionPreview shows title and subtitle only).
    const headerSub = document.createElement('div');
    headerSub.className = 'preview-header-sub';
    const detail = document.createElement('span');
    detail.className = 'preview-size';
    detail.textContent = result.subtitle || sourceblocks.blockName(result.id) || '';
    headerSub.appendChild(detail);
    headerText.appendChild(headerSub);
    header.appendChild(headerText);
    column.appendChild(header);

    const body = document.createElement('div');
    body.className = 'preview-slot';
    column.appendChild(body);

    // Pushed to the bottom by its own margin, so a block with two steps and a
    // block with a long preview both say where they came from in the same place.
    const footer = document.createElement('div');
    footer.className = 'preview-block-footer';
    column.appendChild(footer);

    const block = await sourceblocks.loadDetail(result);
    if (!block || currentPath !== cacheKey) return;

    if (block.steps.length > 0) {
        // Above the scrolling region, not inside it: what Enter does should not
        // scroll away from the commands it labels (macOS keeps the same line
        // outside its ScrollView).
        const label = document.createElement('div');
        label.className = 'preview-section-label';
        label.textContent = 'Enter runs';
        column.insertBefore(label, body);
        body.appendChild(await stepsBlock(block.steps));
        if (currentPath !== cacheKey) return;
    }

    if (block.file) {
        const divider = document.createElement('div');
        divider.className = 'preview-divider';
        footer.appendChild(divider);

        const meta = document.createElement('div');
        meta.className = 'preview-meta';
        meta.appendChild(infoRow('Declared in', sourceblocks.tildePath(block.file)));
        footer.appendChild(meta);

        // A row that names its own path reveals that, like every other row with
        // one, so the chord belongs to the declaration only when the row has
        // nothing of its own to point at.
        if (!result.path) footer.appendChild(hintRow('Ctrl+F', 'Reveal that file'));
    }

    // The declared `preview` runs a command, so it is read last and only after
    // the cheap details are on screen.
    const preview = await readSourcePreview(result);
    if (!preview || currentPath !== cacheKey) return;
    // Only real output shares the space: a block with none leaves the steps to
    // fill it, and a failure is one line rather than a second pane.
    if (!preview.error && preview.text) column.classList.add('has-preview');
    body.appendChild(blockPreviewBody(preview));
}

/** The declared `preview` runs a command, so the backend can refuse to: a
 *  rejection leaves the panel without that section rather than unhandled. */
async function readSourcePreview(result) {
    try {
        return await sourcePreview(sourceblocks.rowPayload(result));
    } catch (err) {
        console.warn('preview: could not read a block preview', err);
        return null;
    }
}

/** The steps ARE shell, so they get what an AI answer's code gets: highlighted,
 *  selectable, and copyable in one press. */
async function stepsBlock(steps) {
    const source = steps.join('\n');
    const wrap = document.createElement('div');
    wrap.className = 'preview-code preview-steps';

    const copyButton = document.createElement('button');
    copyButton.type = 'button';
    copyButton.className = 'ai-card-copy preview-steps-copy';
    copyButton.tabIndex = -1;
    copyButton.title = 'Copy these steps';
    copyButton.innerHTML = copyIcon;
    copyButton.addEventListener('click', () => {
        copyToClipboard(source)
            .then(() => banner.show('Steps copied', 'success', 1.0))
            .catch(() => banner.show('Copy failed', 'error', 1.2));
    });
    wrap.appendChild(copyButton);

    const pre = document.createElement('pre');
    pre.className = 'preview-code-text';
    // Plain text first: highlighting is a round trip, and the steps must be on
    // screen either way.
    pre.textContent = source;
    wrap.appendChild(pre);
    try {
        const highlighted = await highlightShell(source);
        if (highlighted?.html) pre.innerHTML = highlighted.html;
    } catch {
        // The unhighlighted text is already there and says the same thing.
    }
    return wrap;
}

/** A block's `preview` output, or the reason it could not run. A failure is
 *  shown rather than swallowed: a preview that silently does nothing reads as
 *  the feature being broken. */
function blockPreviewBody(preview) {
    if (preview.error) {
        const failed = document.createElement('div');
        failed.className = 'preview-code-truncated';
        failed.textContent = preview.error;
        return failed;
    }
    const wrap = document.createElement('div');
    wrap.className = 'preview-code';
    const pre = document.createElement('pre');
    pre.className = 'preview-code-text';
    pre.textContent = preview.text;
    wrap.appendChild(pre);
    return wrap;
}

/** A chord and what it does: the key in a cap, the wording beside it. Not an
 *  info row - that is a fact about the thing, this is something you can press. */
function hintRow(key, text) {
    const row = document.createElement('div');
    row.className = 'preview-hint';

    const cap = document.createElement('span');
    cap.className = 'preview-hint-key';
    cap.textContent = key;
    row.appendChild(cap);

    const label = document.createElement('span');
    label.className = 'preview-hint-text';
    label.textContent = text;
    row.appendChild(label);

    return row;
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

// Mirrors macOS WebSuggestionPreviewView.swift - a centred "search the web"
// card. A web-suggestion row has no file metadata to render, so we show the
// action affordance instead.
function renderWebSuggestionPreview(query) {
    const wrap = document.createElement('div');
    wrap.className = 'preview-web-suggestion';

    const icon = document.createElement('div');
    icon.className = 'preview-web-suggestion-icon';
    icon.innerHTML = `<svg width="44" height="44" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>`;
    wrap.appendChild(icon);

    const title = document.createElement('div');
    title.className = 'preview-web-suggestion-title';
    title.textContent = query;
    wrap.appendChild(title);

    const subtitle = document.createElement('div');
    subtitle.className = 'preview-web-suggestion-subtitle';
    subtitle.textContent = 'Search Google';
    wrap.appendChild(subtitle);

    const hint = document.createElement('div');
    hint.className = 'preview-web-suggestion-hint';
    hint.innerHTML = `Press <kbd>Enter</kbd> to search the web`;
    wrap.appendChild(hint);

    panel.appendChild(wrap);
}

// Same layout again, with the expression as the subtitle.
function renderCalcPreview(result) {
    const wrap = document.createElement('div');
    wrap.className = 'preview-web-suggestion';

    const icon = document.createElement('div');
    icon.className = 'preview-web-suggestion-icon';
    icon.innerHTML = calculatorLg;
    wrap.appendChild(icon);

    const title = document.createElement('div');
    title.className = 'preview-web-suggestion-title';
    title.textContent = result.title;
    wrap.appendChild(title);

    const sub = document.createElement('div');
    sub.className = 'preview-web-suggestion-subtitle';
    sub.textContent = result.calcExpr || '';
    wrap.appendChild(sub);

    const hint = document.createElement('div');
    hint.className = 'preview-web-suggestion-hint';
    hint.innerHTML = `Press <kbd>Enter</kbd> to copy`;
    wrap.appendChild(hint);

    panel.appendChild(wrap);
}

// URL rows share the web-suggestion preview layout/classes; only the glyph,
// subtitle and hint differ, so no new CSS.
function renderWebUrlPreview(url, subtitle) {
    const wrap = document.createElement('div');
    wrap.className = 'preview-web-suggestion';

    const icon = document.createElement('div');
    icon.className = 'preview-web-suggestion-icon';
    icon.innerHTML = globeLg;
    wrap.appendChild(icon);

    const title = document.createElement('div');
    title.className = 'preview-web-suggestion-title';
    title.textContent = url;
    wrap.appendChild(title);

    const sub = document.createElement('div');
    sub.className = 'preview-web-suggestion-subtitle';
    sub.textContent = subtitle;
    wrap.appendChild(sub);

    const hint = document.createElement('div');
    hint.className = 'preview-web-suggestion-hint';
    hint.innerHTML = `Press <kbd>Enter</kbd> to open in browser`;
    wrap.appendChild(hint);

    panel.appendChild(wrap);
}
