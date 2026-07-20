import assert from 'node:assert/strict';
import test from 'node:test';

import {
    processSearchScore,
    rankProcesses,
} from '../src/js/screens/commands/process-search.mjs';

test('matches process names by fuzzy subsequence', () => {
    assert.notEqual(
        processSearchScore('vsc', { name: 'Visual Studio Code', pid: 42 }),
        null,
    );
    assert.notEqual(
        processSearchScore('visual studio', { name: 'Visual Studio Code', pid: 42 }),
        null,
    );
});

test('matches process ids and executable paths', () => {
    assert.notEqual(processSearchScore('424', { name: 'Terminal', pid: 4242 }), null);
    assert.notEqual(
        processSearchScore('firefx', {
            name: 'Web Browser',
            pid: 7,
            exec: '/usr/bin/firefox',
        }),
        null,
    );
});

test('ranks exact matches above fuzzy matches while preserving stable ties', () => {
    const exact = { name: 'Code', pid: 1 };
    const fuzzy = { name: 'Visual Studio Code', pid: 2 };
    const results = rankProcesses('code', [fuzzy, exact]);
    assert.deepEqual(results, [exact, fuzzy]);
});

test('rejects unrelated processes', () => {
    assert.equal(processSearchScore('xyz', { name: 'Terminal', pid: 42 }), null);
});
