import * as layout from '../layout.js';
import { translate, translateLanguages, copyToClipboard } from '../ipc.js';
import { globeLg, copy as copyIcon, link as linkIcon, externalLink } from '../icons.js';

// Last resort when the backend call fails; the real list, labels included,
// comes from `translate_languages`.
const DEFAULT_LANGUAGES = [
    { code: 'vi', label: 'TIẾNG VIỆT' },
    { code: 'en', label: 'ENGLISH' },
    { code: 'ja', label: '日本語' },
];

// The target languages come from `translate_languages` in the config, in order.
// The backend parses and validates them (dropping empty, duplicate, or
// unsupported codes, and falling back to the defaults), so every shell shares
// one table. Labels are uppercased for the section headers.
async function configuredLanguages() {
    try {
        const languages = await translateLanguages();
        return languages.map((lang) => ({
            code: lang.code,
            label: lang.label.toUpperCase(),
        }));
    } catch {
        return DEFAULT_LANGUAGES;
    }
}

let container = null;
let active = false;
// Monotonic token identifying the newest `perform` call. Overlapping calls
// bump it, so a stale call can detect it has been superseded and stop before
// touching the panel.
let performToken = 0;

export function init(containerEl) {
    container = containerEl;
}

export function isActive() {
    return active;
}

// Floating, layout.js parks the hint text here; collapsed otherwise.
function hintSlot() {
    const slot = document.createElement('div');
    slot.className = 'pane-footer';
    return slot;
}

export function showPlaceholder() {
    hide();
    active = true;
    const panel = document.createElement('div');
    panel.className = 'translate-panel pane-tile';
    const placeholder = document.createElement('div');
    placeholder.className = 'translate-placeholder';
    placeholder.innerHTML =
        '<div class="translate-placeholder-icon">' +
        globeLg +
        '</div>' +
        '<div class="translate-placeholder-text">Press Enter to translate</div>';
    panel.appendChild(placeholder);
    panel.appendChild(hintSlot());
    container.appendChild(panel);
    layout.refresh();
}

export function hide() {
    active = false;
    performToken += 1;
    const panel = container.querySelector('.translate-panel');
    if (panel) {
        layout.releaseHints();
        panel.remove();
        layout.refresh();
    }
}

export async function perform(text) {
    const token = (performToken += 1);
    if (!text.trim()) return;
    active = true;

    const languages = await configuredLanguages();
    if (token !== performToken) return;

    // Remove old panel
    let panel = container.querySelector('.translate-panel');
    if (panel) {
        layout.releaseHints();
        panel.remove();
    }

    panel = document.createElement('div');
    panel.className = 'translate-panel pane-tile';
    container.appendChild(panel);
    // Source header: bold text + WEB badge
    const sourceHeader = document.createElement('div');
    sourceHeader.className = 'translate-source';
    const sourceLeft = document.createElement('div');
    sourceLeft.className = 'translate-source-left';
    const sourceText = document.createElement('div');
    sourceText.className = 'translate-source-text';
    sourceText.textContent = text;
    sourceLeft.appendChild(sourceText);
    const webBadge = document.createElement('span');
    webBadge.className = 'translate-web-badge';
    webBadge.textContent = 'WEB';
    sourceLeft.appendChild(webBadge);
    sourceHeader.appendChild(sourceLeft);
    panel.appendChild(sourceHeader);

    // Language sections (show loading state)
    const sections = languages.map((lang) => {
        const section = document.createElement('div');
        section.className = 'translate-section';

        const header = document.createElement('div');
        header.className = 'translate-section-header';

        const label = document.createElement('span');
        label.className = 'translate-lang-label';
        label.textContent = lang.label;
        header.appendChild(label);

        const actions = document.createElement('div');
        actions.className = 'translate-section-actions';

        const copyBtn = document.createElement('button');
        copyBtn.className = 'translate-icon-btn';
        copyBtn.title = 'Copy';
        copyBtn.innerHTML = copyIcon;
        copyBtn.disabled = true;
        actions.appendChild(copyBtn);

        header.appendChild(actions);
        section.appendChild(header);

        const body = document.createElement('div');
        body.className = 'translate-section-body';
        body.textContent = 'Translating\u2026';
        section.appendChild(body);

        panel.appendChild(section);
        return { lang, section, body, copyBtn };
    });

    // Footer: Open in Browser - pinned to bottom
    const footer = document.createElement('div');
    footer.className = 'translate-footer';
    footer.addEventListener('click', () => {
        const url = `https://translate.google.com/?text=${encodeURIComponent(text)}&sl=auto&tl=${encodeURIComponent(languages[0].code)}`;
        window.__TAURI__.core.invoke('open_path', { path: url, kind: 'browser', id: '' });
    });
    footer.innerHTML =
        '<span class="translate-footer-icon">' +
        linkIcon +
        '</span>' +
        '<span class="translate-footer-text">Open in Browser</span>' +
        '<span class="translate-footer-arrow">' +
        externalLink +
        '</span>';
    panel.appendChild(footer);
    panel.appendChild(hintSlot());
    layout.refresh();

    // Translate every configured language in parallel
    const results = await Promise.allSettled(languages.map((lang) => translate(text, lang.code)));
    if (token !== performToken) return;

    results.forEach((res, i) => {
        const { body, copyBtn } = sections[i];
        if (res.status === 'fulfilled' && !res.value.error) {
            body.textContent = res.value.translated;
            body.classList.add('translate-success');
            copyBtn.disabled = false;
            copyBtn.addEventListener('click', () => {
                copyToClipboard(res.value.translated);
            });
        } else {
            const errMsg = res.status === 'fulfilled' ? res.value.error : 'Translation failed';
            body.textContent = errMsg;
            body.classList.add('translate-error');
        }
    });
}
