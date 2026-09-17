// A key press in core's hotkey grammar (`ctrl+shift+k`). Letters follow the
// printed layout; digits and named keys follow the physical key.

const SPEC_SEPARATOR = '+';
const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'AltGraph', 'Meta', 'OS']);
const KEY_CODE_PREFIX = 'Key';
const DIGIT_CODE_PREFIX = 'Digit';
const SINGLE_LETTER = /^[a-z]$/i;

export function isModifierOnly(e) {
    return MODIFIER_KEYS.has(e.key);
}

export function specFor(e) {
    const parts = [];
    if (e.ctrlKey) parts.push('ctrl');
    if (e.altKey) parts.push('alt');
    if (e.shiftKey) parts.push('shift');
    if (e.metaKey) parts.push('win');
    parts.push(keyToken(e));
    return parts.join(SPEC_SEPARATOR);
}

function keyToken(e) {
    if (SINGLE_LETTER.test(e.key)) return e.key.toLowerCase();
    if (e.code.startsWith(KEY_CODE_PREFIX)) {
        return e.code.slice(KEY_CODE_PREFIX.length).toLowerCase();
    }
    if (e.code.startsWith(DIGIT_CODE_PREFIX)) return e.code.slice(DIGIT_CODE_PREFIX.length);
    return e.code.toLowerCase();
}
