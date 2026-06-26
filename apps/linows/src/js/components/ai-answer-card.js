// Renders the AI / web answer card. Pure view layer — subscribes to
// ai-answer.js via onChange and re-paints on every state mutation.
//
// Matches macOS AIAnswerCardView.swift:
//   - 10px corner radius, 12px padding, 1px white-8% border, control-fill bg
//   - header row: sparkles icon + question + spinner (when streaming)
//   - one block per source, 14px vertical spacing:
//       SOURCE LABEL (uppercased, accent-colored if clickable, with chevron)
//       copy button on the right
//       optional 96x96 image (clickable → opens url)
//       answer text (selectable, generous line height)
//   - status line: "Thinking…" while streaming with no blocks yet;
//                  "Couldn't find an answer." on failed.

import { sparkles, copy as copyIcon, arrowUpRight } from '../icons.js';
import { copyToClipboard, openPath } from '../ipc.js';
import * as banner from './banner.js';

let cardEl = null;

export function init(el) {
  cardEl = el;
}

export function update(snapshot) {
  if (!cardEl) return;
  const { state, question, items } = snapshot;

  if (state === 'idle') {
    cardEl.hidden = true;
    cardEl.innerHTML = '';
    return;
  }

  const headerLabel = question || 'Web answer';
  const streamingDot = state === 'streaming' ? '<span class="ai-spinner" aria-label="Loading"></span>' : '';

  const blocks = items.map(renderBlock).join('');
  const status = renderStatusLine(state, items.length === 0);

  cardEl.hidden = false;
  cardEl.innerHTML = `
    <div class="ai-card-header">
      <span class="ai-card-icon">${sparkles}</span>
      <span class="ai-card-question">${escapeHtml(headerLabel)}</span>
      <span class="ai-card-spacer"></span>
      ${streamingDot}
    </div>
    <div class="ai-card-body">
      ${blocks}
      ${status}
    </div>
  `;

  wireBlockHandlers();
}

function renderBlock(item) {
  const hasUrl = !!item.url;
  const sourceClass = hasUrl ? 'ai-card-source ai-card-source-linked' : 'ai-card-source';
  const chevron = hasUrl ? `<span class="ai-card-source-chevron">${arrowUpRight}</span>` : '';
  const image = item.imageUrl
    ? `<img class="ai-card-image" src="${escapeAttr(item.imageUrl)}" alt="" data-url="${escapeAttr(item.url || '')}" />`
    : '';

  return `
    <div class="ai-card-block">
      <div class="ai-card-block-head">
        <span class="${sourceClass}" data-url="${escapeAttr(item.url || '')}">
          <span class="ai-card-source-label">${escapeHtml(item.source.toUpperCase())}</span>
          ${chevron}
        </span>
        <span class="ai-card-spacer"></span>
        <button type="button" class="ai-card-copy" data-text="${escapeAttr(item.text)}" title="Copy this answer" tabindex="-1">${copyIcon}</button>
      </div>
      <div class="ai-card-block-body">
        ${image}
        <div class="ai-card-text">${escapeHtml(item.text)}</div>
      </div>
    </div>
  `;
}

function renderStatusLine(state, isEmpty) {
  if (!isEmpty) return '';
  if (state === 'streaming') return `<div class="ai-card-status">Thinking…</div>`;
  if (state === 'failed') return `<div class="ai-card-status">Couldn't find an answer.</div>`;
  return '';
}

function wireBlockHandlers() {
  cardEl.querySelectorAll('.ai-card-source-linked').forEach((el) => {
    el.addEventListener('click', () => {
      const url = el.dataset.url;
      if (url) openPath(url, 'browser', '');
    });
  });
  cardEl.querySelectorAll('.ai-card-image[data-url]').forEach((el) => {
    el.addEventListener('click', () => {
      const url = el.dataset.url;
      if (url) openPath(url, 'browser', '');
    });
  });
  cardEl.querySelectorAll('.ai-card-copy').forEach((el) => {
    el.addEventListener('click', async (e) => {
      e.stopPropagation();
      const text = (el.dataset.text || '').trim();
      if (!text) return;
      try {
        await copyToClipboard(text);
        banner.show('Copied answer', 'success', 1.0);
      } catch {
        banner.show('Copy failed', 'error', 1.2);
      }
    });
  });
}

function escapeHtml(s) {
  return String(s ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

function escapeAttr(s) {
  return escapeHtml(s).replace(/"/g, '&quot;');
}
