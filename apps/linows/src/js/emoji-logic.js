// Pure /emoji recents logic, kept apart from the DOM so it can be tested.
export const RECENTS_CAP = 20;
const KEY = 'look.emoji.recents';

export function pushRecent(list, entry, cap = RECENTS_CAP) {
    return [entry, ...list.filter((e) => e.glyph !== entry.glyph)].slice(0, cap);
}

export function loadRecents(storage) {
    try {
        const parsed = JSON.parse(storage.getItem(KEY));
        if (!Array.isArray(parsed)) return [];
        return parsed
            .filter((e) => e && typeof e.glyph === 'string' && typeof e.name === 'string')
            .slice(0, RECENTS_CAP);
    } catch {
        return [];
    }
}

export function saveRecents(storage, list) {
    storage.setItem(KEY, JSON.stringify(list));
}
