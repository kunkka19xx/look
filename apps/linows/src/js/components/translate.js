import * as layout from '../layout.js';
import { translate, copyToClipboard, getConfig } from '../ipc.js';
import { globeLg, copy as copyIcon, link as linkIcon, externalLink } from '../icons.js';

const DEFAULT_LANGUAGES = [
    { code: 'vi', label: 'TIẾNG VIỆT' },
    { code: 'en', label: 'ENGLISH' },
    { code: 'ja', label: '日本語' },
];

// Display names for the supported codes. Keys are lowercase because the config
// value is lowercased before lookup (so `zh-CN` reads as `zh-cn`). This doubles
// as the allowlist: a code with no entry is a typo or an unsupported language,
// so it is dropped rather than shown with a placeholder label.
const LANGUAGE_LABELS = {
    af: 'Afrikaans',
    ar: 'العربية',
    az: 'Azərbaycan',
    be: 'Беларуская',
    bg: 'Български',
    bn: 'বাংলা',
    ca: 'Català',
    cs: 'Čeština',
    da: 'Dansk',
    de: 'Deutsch',
    el: 'Ελληνικά',
    en: 'English',
    es: 'Español',
    et: 'Eesti',
    eu: 'Euskara',
    fa: 'فارسی',
    fi: 'Suomi',
    fil: 'Filipino',
    fr: 'Français',
    gl: 'Galego',
    gu: 'ગુજરાતી',
    he: 'עברית',
    hi: 'हिन्दी',
    hr: 'Hrvatski',
    hu: 'Magyar',
    hy: 'Հայերեն',
    id: 'Bahasa Indonesia',
    is: 'Íslenska',
    it: 'Italiano',
    ja: '日本語',
    ka: 'ქართული',
    kk: 'Қазақ',
    km: 'ខ្មែរ',
    kn: 'ಕನ್ನಡ',
    ko: '한국어',
    lo: 'ລາວ',
    lt: 'Lietuvių',
    lv: 'Latviešu',
    mk: 'Македонски',
    ml: 'മലയാളം',
    mn: 'Монгол',
    mr: 'मराठी',
    ms: 'Bahasa Melayu',
    my: 'မြန်မာ',
    ne: 'नेपाली',
    nl: 'Nederlands',
    no: 'Norsk',
    pa: 'ਪੰਜਾਬੀ',
    pl: 'Polski',
    pt: 'Português',
    'pt-br': 'Português (Brasil)',
    ro: 'Română',
    ru: 'Русский',
    si: 'සිංහල',
    sk: 'Slovenčina',
    sl: 'Slovenščina',
    sq: 'Shqip',
    sr: 'Српски',
    sv: 'Svenska',
    sw: 'Kiswahili',
    ta: 'தமிழ்',
    te: 'తెలుగు',
    th: 'ไทย',
    tr: 'Türkçe',
    uk: 'Українська',
    ur: 'اردو',
    uz: 'Oʻzbek',
    vi: 'Tiếng Việt',
    zh: '中文',
    'zh-cn': '中文（简体）',
    'zh-tw': '中文（繁體）',
};

// The target languages come from `translate_languages` in the config, in order.
// Codes that are empty, duplicated, or not in LANGUAGE_LABELS are dropped from
// both the label list and the result sections; an absent, empty, or all-invalid
// list falls back to the defaults, so a typo never blanks the panel.
async function configuredLanguages() {
    try {
        const cfg = await getConfig();
        const raw = cfg?.entries?.find((e) => e.key === 'translate_languages')?.value ?? '';
        const seen = new Set();
        const languages = [];
        for (const entry of raw.split(',')) {
            const code = entry.trim().toLowerCase();
            const label = LANGUAGE_LABELS[code];
            if (!code || seen.has(code) || typeof label !== 'string') continue;
            seen.add(code);
            languages.push({ code, label: label.toUpperCase() });
        }
        return languages.length ? languages : DEFAULT_LANGUAGES;
    } catch {
        return DEFAULT_LANGUAGES;
    }
}

let container = null;
let active = false;

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
    const panel = container.querySelector('.translate-panel');
    if (panel) {
        layout.releaseHints();
        panel.remove();
        layout.refresh();
    }
}

export async function perform(text) {
    if (!text.trim()) return;
    active = true;

    const languages = await configuredLanguages();

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
