import { translate, copyToClipboard } from '../ipc.js';
import { globeLg, copy as copyIcon, link as linkIcon, externalLink } from '../icons.js';

const LANGUAGES = [
  { code: 'vi', label: 'TIẾNG VIỆT' },
  { code: 'en', label: 'ENGLISH' },
  { code: 'ja', label: '日本語' },
];

let container = null;
let active = false;

export function init(containerEl) {
  container = containerEl;
}

export function isActive() {
  return active;
}

export function showPlaceholder() {
  hide();
  active = true;
  const panel = document.createElement('div');
  panel.className = 'translate-panel';
  const placeholder = document.createElement('div');
  placeholder.className = 'translate-placeholder';
  placeholder.innerHTML = '<div class="translate-placeholder-icon">' + globeLg + '</div>' +
    '<div class="translate-placeholder-text">Press Enter after finishing input to translate on web</div>';
  panel.appendChild(placeholder);
  container.appendChild(panel);
}

export function hide() {
  active = false;
  const panel = container.querySelector('.translate-panel');
  if (panel) panel.remove();
}

export async function perform(text) {
  if (!text.trim()) return;
  active = true;

  // Remove old panel
  let panel = container.querySelector('.translate-panel');
  if (panel) panel.remove();

  panel = document.createElement('div');
  panel.className = 'translate-panel';
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
  const sections = LANGUAGES.map((lang) => {
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
    const url = `https://translate.google.com/?text=${encodeURIComponent(text)}&sl=auto&tl=en`;
    window.__TAURI__.core.invoke('open_path', { path: url, kind: 'browser', id: '' });
  });
  footer.innerHTML =
    '<span class="translate-footer-icon">' + linkIcon + '</span>' +
    '<span class="translate-footer-text">Open in Browser</span>' +
    '<span class="translate-footer-arrow">' + externalLink + '</span>';
  panel.appendChild(footer);

  // Translate all 3 in parallel
  const results = await Promise.allSettled(
    LANGUAGES.map((lang) => translate(text, lang.code)),
  );

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
      const errMsg =
        res.status === 'fulfilled' ? res.value.error : 'Translation failed';
      body.textContent = errMsg;
      body.classList.add('translate-error');
    }
  });
}
