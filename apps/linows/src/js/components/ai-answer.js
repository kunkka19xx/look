// Drives the inline AI / web answer card pinned at the top of the results
// area. Faithful port of macOS AIAnswerController.swift: same trigger
// heuristics, same 350 ms debounce, same source-fan-out + dedup, same state
// names. linows has no on-device LLM, so the streaming fallback that macOS
// uses ("Apple Intelligence" block) is intentionally absent - we surface
// DuckDuckGo + Wikipedia + the pattern-gated instant providers (currency /
// weather / crypto) and stop there.
//
// Arithmetic has its own row now (search.js `fetchCalc`); it used to answer
// here, which is why it needed a question mark to trigger at all.

import {
    instantHasMatch,
    instantAnswer,
    duckduckgoAnswer,
    wikipediaAnswer,
    definitionalEntity,
} from '../ipc.js';

const DEBOUNCE_MS = 350;
// Two answer texts are "the same" when their leading N chars match -
// DuckDuckGo abstracts are often verbatim Wikipedia, so we don't show both.
const SIMILARITY_PREFIX_LEN = 60;

// State machine: same names + meanings as macOS AIAnswerController.State.
//   idle      - not a question / AI off / no answer to show
//   streaming - at least one async source is in flight
//   done      - every source has settled (one or more blocks landed)
//   failed    - every source returned null (only used to show the empty hint)
export const State = Object.freeze({
    idle: 'idle',
    streaming: 'streaming',
    done: 'done',
    failed: 'failed',
});

let state = State.idle;
let question = ''; // The trimmed query backing the current card
let items = []; // Settled blocks, in arrival order
let aiEnabled = true; // Mirrors the config flag

let debounceTimer = null;
let runVersion = 0; // Bumps on every update; in-flight runs check
// this and bail when a newer query supersedes.

let onChangeCallback = null; // View subscribes here; fires after every state
// mutation so the card can re-render.

export function init({ onChange }) {
    onChangeCallback = onChange;
}

export function setEnabled(enabled) {
    aiEnabled = !!enabled;
    if (!aiEnabled) cancel();
}

export function isActive() {
    return state !== State.idle;
}

export function getState() {
    return { state, question, items };
}

// Re-evaluate for the current query. Cancels any in-flight generation.
// `resultCount` is how many local results the launcher found - a multi-word
// entity with no local match (e.g. "david beckham") is treated as a
// knowledge lookup (see isEntityLookup).
export async function update(rawQuery, resultCount) {
    const query = (rawQuery || '').trim();

    // Triggers (mirrors macOS): an explicit question, a multi-word entity with
    // no local match, or a pattern-gated instant source (weather / currency /
    // crypto). instantHasMatch is a network-free regex gate; everything else
    // is local - so this can run on every keystroke without I/O.
    const questionLike = isQuestionLike(query);
    const orphanEntity = resultCount === 0 && isEntityLookup(query);
    const instant = aiEnabled && query.length > 0 ? await instantHasMatch(query) : false;

    if (!aiEnabled || !(questionLike || orphanEntity || instant)) {
        cancel();
        return;
    }

    // Same question already active - leave it be (avoid re-fetch on every
    // keystroke that doesn't change the trimmed query).
    if (query === question && state !== State.idle) return;

    runVersion += 1;
    const myVersion = runVersion;
    question = query;
    items = [];
    state = State.streaming;
    emitChange();

    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(
        () => runFetch(query, questionLike, instant, myVersion),
        DEBOUNCE_MS,
    );
}

// Tear down the card (query cleared, launcher hidden, AI turned off, mode
// switched to clipboard/prefix/command discovery/translate).
export function cancel() {
    runVersion += 1;
    clearTimeout(debounceTimer);
    debounceTimer = null;
    if (state === State.idle && !items.length && !question) return;
    state = State.idle;
    question = '';
    items = [];
    emitChange();
}

// ---- internals ----

function emitChange() {
    if (onChangeCallback) onChangeCallback(getState());
}

// A run is "stale" when a newer update() has bumped runVersion. Every async
// step checks this before mutating state, so a slow fetch from a previous
// query can't paint over the answer for a later one.
function isStale(myVersion) {
    return myVersion !== runVersion;
}

async function runFetch(query, questionLike, instant, myVersion) {
    if (isStale(myVersion)) return;

    // Choose what (if anything) to search Wikipedia for. Mirrors macOS
    // collectWebAnswers: definitionalEntity for "what is X" patterns; the raw
    // query for bare entities; nothing for how-to/why questions.
    let wikiTerm = null;
    if (!instant) {
        const entity = await definitionalEntity(query).catch(() => null);
        if (isStale(myVersion)) return;
        if (entity) wikiTerm = entity;
        else if (!questionLike) wikiTerm = query;
    }

    // Fan out concurrently. A matched instant source (currency / weather /
    // crypto) is what the user wants - skip the generic encyclopedia lookups
    // then. Each task is a Promise<Item | null> that the loop below appends as
    // it resolves so the first available source surfaces immediately, and a
    // single slow provider doesn't hold up the others.
    const tasks = instant
        ? [instantAnswer(query).then(toItem)]
        : [
              duckduckgoAnswer(query).then(toItem),
              ...(wikiTerm ? [wikipediaAnswer(wikiTerm).then(toItem)] : []),
          ];

    await collect(tasks, myVersion);
    if (isStale(myVersion)) return;

    // An instant provider that came back empty means its API failed, not that
    // there's nothing to say.
    if (instant && items.length === 0) {
        await collect(
            [
                duckduckgoAnswer(query).then(toItem),
                ...(wikiTerm ? [wikipediaAnswer(wikiTerm).then(toItem)] : []),
            ],
            myVersion,
        );
        if (isStale(myVersion)) return;
    }

    state = items.length ? State.done : State.failed;
    emitChange();
}

// Appends each answer as it lands, so the first source to resolve paints
// immediately. Duplicate sources and near-identical text are dropped.
async function collect(tasks, myVersion) {
    await Promise.all(
        tasks.map(async (p) => {
            const result = await p;
            if (isStale(myVersion) || !result) return;
            if (items.some((it) => it.source === result.source)) return;
            if (items.some((it) => similar(it.text, result.text))) return;
            items = [...items, result];
            state = State.streaming;
            emitChange();
        }),
    );
}

// Normalise the Rust `Answer` (snake_case wire shape) to the controller's
// item shape used by the card view.
function toItem(answer) {
    if (!answer) return null;
    return {
        text: answer.text,
        source: answer.source,
        url: answer.url || null,
        imageUrl: answer.image_url || null,
    };
}

function similar(a, b) {
    const key = (s) => s.toLowerCase().replace(/\s+/g, '').slice(0, SIMILARITY_PREFIX_LEN);
    return key(a) === key(b);
}

// Cheap heuristic for "this looks like a question, not an app/file launch".
// Keeps the network off the hot path for ordinary launches like "spotify".
// Matches macOS AIAnswerController.isQuestionLike one-for-one.
const QUESTION_STARTERS = new Set([
    'how',
    'what',
    'why',
    'who',
    'when',
    'where',
    'which',
    'whose',
    'can',
    'could',
    'should',
    'would',
    'is',
    'are',
    'am',
    'do',
    'does',
    'did',
    'will',
    'explain',
    'tell',
    'give',
    'write',
    'summarize',
    'summarise',
    'define',
    'translate',
    'convert',
    'calculate',
]);

export function isQuestionLike(text) {
    if (text.length < 3) return false;
    if (text.endsWith('?')) return true;
    const words = text.split(/[\s\n]+/).filter(Boolean);
    if (words.length < 3) return false;
    return QUESTION_STARTERS.has(words[0].toLowerCase());
}

// Multi-word, reasonably long query that's likely a name/entity rather than
// a half-typed token. Combined with zero local results, this marks a
// knowledge lookup ("david beckham") vs. an app launch ("activity monitor").
export function isEntityLookup(text) {
    if (text.length < 5) return false;
    const words = text.split(/[\s\n]+/).filter(Boolean);
    return words.length >= 2;
}
