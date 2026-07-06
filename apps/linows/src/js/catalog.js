// Single source of truth for query prefixes and slash commands, mirroring
// macOS AppConstants.swift (PrefixSuggestion + commandCatalog) so the
// `"` menu, the `:` menu, and the Help screen can't drift. linows omits
// `tw"` (no dictionary lookup yet).

import { calculator, timer, listChecks, xCircle, terminal, info } from './icons.js';

// Synthetic-row id namespaces: the renderer and Enter/click handlers tell
// synthetic rows apart from real candidates by id prefix.
const PREFIX_HINT_ID = 'prefixhint:';
const COMMAND_HINT_ID = 'cmdhint:';
const WEB_SUGGEST_ID = 'websuggest:';
const DISCOVERY_CHAR = '"';

// Google-autocomplete row glyph: Lucide `search` (mirrors macOS, which uses
// SF Symbols "magnifyingglass"). Inlined here rather than imported as a
// dedicated icon constant since this is the only place that needs it.
const SEARCH_GLYPH = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>`;

const PREFIX_ENTRIES = [
  { prefix: 'a"',  argHint: 'word',    description: 'Apps only' },
  { prefix: 'f"',  argHint: 'word',    description: 'Files only' },
  { prefix: 'd"',  argHint: 'word',    description: 'Folders only' },
  { prefix: 'rc"', argHint: 'word',    description: 'Recent files/folders, newest first (optional filter)' },
  { prefix: 'r"',  argHint: 'pattern', description: 'Regex search' },
  { prefix: 'c"',  argHint: 'word',    description: 'Clipboard history search (latest 10 text clips)' },
  { prefix: 't"',  argHint: 'word',    description: 'Web translate (VI/EN/JA)' },
];

// Shortcut numbers must match the Ctrl+1..6 bindings in
// screens/commands/index.js, otherwise the title hint lies to the user.
const COMMAND_ENTRIES = [
  { id: 'calc',  title: 'calc (Ctrl+1)',  detail: 'Evaluate math expression',          icon: calculator },
  { id: 'pomo',  title: 'pomo (Ctrl+2)',  detail: 'Pomodoro focus timer',              icon: timer },
  { id: 'todo',  title: 'todo (Ctrl+3)',  detail: 'Daily tasks & progress',            icon: listChecks },
  { id: 'kill',  title: 'kill (Ctrl+4)',  detail: 'Force kill app or process by port', icon: xCircle },
  { id: 'shell', title: 'shell (Ctrl+5)', detail: 'Run a shell command',               icon: terminal },
  { id: 'sys',   title: 'sys (Ctrl+6)',   detail: 'Show system information',           icon: info },
];

// True when `:cmd <ws>` should bypass the discovery menu and live-trigger
// the command panel (matches macOS extractInlineCommand). Bare `:calc` keeps
// the menu open; only whitespace after a known id flips the trigger.
function isInlineCommandWithArgs(query) {
  if (!query.startsWith(':')) return false;
  const spaceIdx = query.slice(1).search(/\s/);
  if (spaceIdx < 0) return false;
  const id = query.slice(1, 1 + spaceIdx).toLowerCase();
  return COMMAND_ENTRIES.some((c) => c.id === id);
}

export function isPrefixSuggestionQuery(query) {
  return query.trimStart().startsWith(DISCOVERY_CHAR);
}

// True when the query is in any prefix-driven mode: the `"`/`:` discovery
// menus, an inline `:cmd <args>`, or one of the PREFIX_ENTRIES (a"/f"/d"/
// rc"/r"/c"/t"). Web suggestions and AI/Wikipedia lookups are gated off
// for these. The query is a launcher scope, not a knowledge question.
// Adding a new prefix to PREFIX_ENTRIES picks up everywhere automatically.
export function isPrefixedQuery(query) {
  const trimmed = (query || '').trimStart();
  if (!trimmed) return false;
  if (trimmed.startsWith(DISCOVERY_CHAR) || trimmed.startsWith(':')) return true;
  return PREFIX_ENTRIES.some((e) => trimmed.startsWith(e.prefix));
}

export function isCommandSuggestionQuery(query) {
  const trimmed = query.trimStart();
  return trimmed.startsWith(':') && !isInlineCommandWithArgs(trimmed);
}

// Synthetic rows backing the `"` menu, narrowed by what the user typed after
// the leading `"`. Case-insensitive substring match against prefix, display
// form, and description, so `"folder` finds `d"` by intent rather than only
// by the cryptic prefix letter.
export function prefixSuggestionResults(query) {
  const filter = query.trimStart().slice(DISCOVERY_CHAR.length).trim().toLowerCase();
  const entries = filter
    ? PREFIX_ENTRIES.filter((e) =>
        e.prefix.toLowerCase().includes(filter)
        || (e.prefix + e.argHint).toLowerCase().includes(filter)
        || e.description.toLowerCase().includes(filter))
    : PREFIX_ENTRIES;
  return entries.map((entry, index) => ({
    id: `${PREFIX_HINT_ID}${entry.prefix}`,
    kind: 'app',
    title: entry.prefix + entry.argHint,
    subtitle: entry.description,
    path: '',
    score: entries.length - index,
  }));
}

// Synthetic rows backing the `:` menu. `:end` or `:process` both surface
// `kill`. Each row carries its command's icon so the list scans visually.
export function commandSuggestionResults(query) {
  const filter = query.trimStart().slice(1).trim().toLowerCase();
  const entries = filter
    ? COMMAND_ENTRIES.filter((e) =>
        e.id.toLowerCase().includes(filter)
        || e.detail.toLowerCase().includes(filter))
    : COMMAND_ENTRIES;
  return entries.map((entry, index) => ({
    id: `${COMMAND_HINT_ID}${entry.id}`,
    kind: 'app',
    title: entry.title,
    subtitle: entry.detail,
    path: '',
    score: entries.length - index,
    iconSvg: entry.icon,
  }));
}

export function prefixFromResultId(resultId) {
  return resultId?.startsWith(PREFIX_HINT_ID) ? resultId.slice(PREFIX_HINT_ID.length) : null;
}

export function commandIdFromResultId(resultId) {
  return resultId?.startsWith(COMMAND_HINT_ID) ? resultId.slice(COMMAND_HINT_ID.length) : null;
}

// Google-autocomplete rows. Sit at the bottom of the results list when AI is
// on and the query is plain text (`>=2` chars, no mode prefix). Enter on one
// opens Google for that text. Mirrors macOS AppConstants.Launcher.WebSuggestion.
export function webSuggestionResults(suggestions) {
  return suggestions.map((text, index) => ({
    id: `${WEB_SUGGEST_ID}${text}`,
    kind: 'app',
    title: text,
    subtitle: 'Search Google',
    path: '',
    score: -1 - index,
    iconSvg: SEARCH_GLYPH,
  }));
}

export function webSuggestionFromResultId(resultId) {
  return resultId?.startsWith(WEB_SUGGEST_ID) ? resultId.slice(WEB_SUGGEST_ID.length) : null;
}
