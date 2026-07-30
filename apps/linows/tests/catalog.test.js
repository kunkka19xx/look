import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
    isSyntheticResultId,
    prefixSuggestionResults,
    commandSuggestionResults,
    webSuggestionResults,
    webUrlResult,
} from '../src/js/catalog.js';

// Guards the Ctrl+Shift+H hide-app path: every synthetic kind:'app' row must be
// rejected so its display text never lands in app_exclude_names.
test('isSyntheticResultId flags every synthetic row id', () => {
    const synthetic = [
        ...prefixSuggestionResults(''),
        ...commandSuggestionResults(':'),
        ...webSuggestionResults(['weather today']),
        webUrlResult('https://example.com', 'Open in browser', 1),
    ];
    for (const row of synthetic) {
        assert.equal(row.kind, 'app', `${row.id} should be a kind:'app' row`);
        assert.equal(isSyntheticResultId(row.id), true, `${row.id} should be synthetic`);
    }
});

test('isSyntheticResultId passes real apps and bad input', () => {
    assert.equal(isSyntheticResultId('app:/usr/share/applications/firefox.desktop'), false);
    assert.equal(isSyntheticResultId(null), false);
    assert.equal(isSyntheticResultId(undefined), false);
    assert.equal(isSyntheticResultId(''), false);
});
