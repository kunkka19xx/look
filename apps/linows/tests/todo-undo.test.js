import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createContext, runInContext } from 'node:vm';
import { webcrypto } from 'node:crypto';

// Exercise the panel's real mutation and shortcut handlers with IPC and
// rendering stubbed, so no Tauri window or database is needed.
const source = readFileSync(new URL('../src/js/screens/commands/todo.js', import.meta.url), 'utf8')
    .replace(/^import\s+[\s\S]*?from\s+'[^']+';\n/gm, '')
    .replace(/^export /gm, '');

function panel(save = async () => {}) {
    const context = createContext({
        structuredClone,
        crypto: webcrypto,
        console,
        todoSave: save,
        checkIcon: '',
        document: { activeElement: null },
    });
    runInContext(
        source +
            `
        renderAll = () => {};
        renderToolbar = () => {};
        showToast = () => {};
        searchInput = { value: '' };
        ensureTodayGroup();
    `,
        context,
    );
    return (code) => runInContext(code, context);
}

test('delete and clear undo restore order, IDs, completion and timestamps', () => {
    const run = panel();
    run(`addTask(todayKey(), 'First'); addTask(todayKey(), 'Second');
        toggleTask(todayKey(), tasksByDay.get(todayKey())[0].id);`);
    const before = run('JSON.stringify([...tasksByDay])');
    run(`removeTask(todayKey(), tasksByDay.get(todayKey())[0].id); undo();`);
    assert.equal(run('JSON.stringify([...tasksByDay])'), before);
    run('clearAll(todayKey()); undo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), before);
});

test('undo all unsaved edits returns to clean state; no-ops add no history', () => {
    const run = panel();
    run(`addTask(todayKey(), 'First');
        editTask(todayKey(), tasksByDay.get(todayKey())[0].id, 'First');
        removeTask(todayKey(), 'missing');
        undo();`);
    assert.equal(run('tasksByDay.get(todayKey()).length'), 0);
    assert.equal(run('isDirty()'), false);
    assert.equal(run('undoHistory.length'), 0);
});

test('restoring the last deleted past task recreates its day', () => {
    const run = panel();
    run(
        `addTask('2020-01-01', 'Past'); removeTask('2020-01-01', tasksByDay.get('2020-01-01')[0].id);`,
    );
    assert.equal(run("tasksByDay.has('2020-01-01')"), false);
    run('undo();');
    assert.equal(run("tasksByDay.get('2020-01-01')[0].name"), 'Past');
});

test('undo crosses a save, relights it, and the next save writes the reverted list', async () => {
    let saved;
    const run = panel(async (tasks) => {
        saved = structuredClone(tasks);
    });
    run(`addTask(todayKey(), 'Keep');`);
    await run('persist()');
    assert.equal(saved.length, 1);
    run('clearAll(todayKey());');
    await run('persist()');
    assert.equal(saved.length, 0);
    assert.equal(run('isDirty()'), false);
    // Back past the save: panel and store disagree again, so Save relights.
    run('undo();');
    assert.equal(run('tasksByDay.get(todayKey())[0].name'), 'Keep');
    assert.equal(run('isDirty()'), true);
    await run('persist()');
    assert.equal(saved.length, 1);
    assert.equal(run('isDirty()'), false);
    run('redo();');
    assert.equal(run('tasksByDay.get(todayKey()).length'), 0);
    assert.equal(run('isDirty()'), true);
});

test('an edit made while a save is in flight stays undoable and dirty', async () => {
    let finish;
    const run = panel(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    run(`addTask(todayKey(), 'Saved');`);
    const pending = run('persist()');
    run(`addTask(todayKey(), 'Not saved');`);
    finish();
    await pending;
    assert.equal(run('isDirty()'), true);
    run('undo();');
    assert.equal(run('tasksByDay.get(todayKey()).length'), 1);
    assert.equal(run('isDirty()'), false);
    run('redo();');
    assert.equal(run('tasksByDay.get(todayKey()).length'), 2);
    assert.equal(run('isDirty()'), true);
});

test('failed save retains undo and dirty state', async () => {
    const run = panel(async () => {
        throw new Error('unavailable');
    });
    run(`addTask(todayKey(), 'Unsaved');`);
    await run('persist()');
    assert.equal(run('isDirty()'), true);
    run('undo();');
    assert.equal(run('isDirty()'), false);
});

test('Ctrl/Cmd+Z and Shift+Z undo/redo without taking text or hidden-panel keys', () => {
    const run = panel();
    run(`visible = true;
        const key = { key: 'z', ctrlKey: true, preventDefault() {} };
        addTask(todayKey(), 'Task');
        document.activeElement = { dataset: { todoField: 'rename' } };`);
    assert.equal(run('handleKey(key)'), false);
    // The search box only filters the list, so it never holds on to Ctrl+Z.
    run('document.activeElement = searchInput; searchInput.value = "query";');
    assert.equal(run('handleKey({ ...key, shiftKey: true })'), false);
    assert.equal(run('handleKey(key)'), true);
    assert.equal(run('isDirty()'), false);
    run('searchInput.value = "";');
    run("document.activeElement = { dataset: { todoField: 'rename' } };");
    assert.equal(run('handleKey({ ...key, shiftKey: true })'), false);
    run('document.activeElement = searchInput;');
    assert.equal(run('handleKey({ ...key, shiftKey: true })'), true);
    assert.equal(run('tasksByDay.get(todayKey())[0].name'), 'Task');
    assert.equal(run('isDirty()'), true);
    assert.equal(run('handleKey(key)'), true);
    assert.equal(run('handleKey({ ...key, ctrlKey: false, metaKey: true, shiftKey: true })'), true);
    run(`addTask(todayKey(), 'Another');`);
    assert.equal(run('handleKey({ ...key, ctrlKey: false, metaKey: true })'), true);
    run(`addTask(todayKey(), 'Hidden'); visible = false;`);
    assert.equal(run('handleKey(key)'), false);
    assert.equal(run('handleKey({ ...key, shiftKey: true })'), false);
});

test('multiple undo/redo steps preserve deleted task data and the saved revision', async () => {
    const run = panel();
    run(`addTask(todayKey(), 'First'); addTask(todayKey(), 'Second');
        toggleTask(todayKey(), tasksByDay.get(todayKey())[0].id);`);
    await run('persist()');
    const saved = run('JSON.stringify([...tasksByDay])');
    run(`editTask(todayKey(), tasksByDay.get(todayKey())[0].id, 'Renamed'); clearAll(todayKey());`);
    const changed = run('JSON.stringify([...tasksByDay])');
    run('undo(); undo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), saved);
    assert.equal(run('isDirty()'), false);
    run('redo(); redo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), changed);
    assert.equal(run('isDirty()'), true);
    run('undo(); undo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), saved);
    assert.equal(run('isDirty()'), false);
    run('clearAll(todayKey());');
    const deleted = run('JSON.stringify([...tasksByDay])');
    run('undo(); redo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), deleted);
    assert.equal(run('isDirty()'), true);
    run('undo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), saved);
});

test('a save made after an undo keeps the redo, which relights Save', async () => {
    const run = panel();
    run(`addTask(todayKey(), 'Discard'); undo();`);
    assert.equal(run('isDirty()'), false);
    assert.equal(run('redoHistory.length'), 1);
    await run('persist()');
    assert.equal(run('isDirty()'), false);
    run('redo();');
    assert.equal(run('tasksByDay.get(todayKey())[0].name'), 'Discard');
    assert.equal(run('isDirty()'), true);
});

test('new edits clear redo, but no-ops and failed saves preserve it', async () => {
    const run = panel(async () => {
        throw new Error('unavailable');
    });
    run(`addTask(todayKey(), 'First'); addTask(todayKey(), 'Second'); undo();
        editTask(todayKey(), tasksByDay.get(todayKey())[0].id, 'First');
        removeTask(todayKey(), 'missing');`);
    await run('persist()');
    assert.equal(run('redoHistory.length'), 1);
    run('redo();');
    assert.equal(run('tasksByDay.get(todayKey())[1].name'), 'Second');
    run(`undo(); addTask(todayKey(), 'Replacement'); redo();`);
    assert.equal(run('redoHistory.length'), 0);
    assert.equal(run('tasksByDay.get(todayKey())[1].name'), 'Replacement');
});

test('an undo made while saving is still redoable afterwards', async () => {
    let finish;
    const run = panel(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    run(`addTask(todayKey(), 'Saved');`);
    const pending = run('persist()');
    run('undo();');
    finish();
    await pending;
    assert.equal(run('isDirty()'), true);
    assert.equal(run('tasksByDay.get(todayKey()).length'), 0);
    assert.equal(run('redoHistory.length'), 1);
    run('redo();');
    assert.equal(run('tasksByDay.get(todayKey())[0].name'), 'Saved');
    assert.equal(run('isDirty()'), false);
});

test('a Save pressed while one is in flight runs again instead of vanishing', async () => {
    const writes = [];
    const gates = [];
    const run = panel(
        (tasks) =>
            new Promise((resolve) => {
                // Joined: arrays built inside the vm realm are not
                // deepStrictEqual to plain ones out here.
                writes.push(tasks.map((t) => t.name).join(','));
                gates.push(resolve);
            }),
    );
    run(`addTask(todayKey(), 'First');`);
    const pending = run('persist()');
    run(`addTask(todayKey(), 'Second'); persist();`);
    gates.shift()();
    await new Promise((resolve) => setImmediate(resolve));
    assert.deepEqual(writes, ['First', 'First,Second']);
    gates.shift()();
    await pending;
    assert.equal(run('isDirty()'), false);
});

test('date placeholders undo in order without dirtying the panel', () => {
    const run = panel();
    run('addDate();');
    assert.equal(run('isDirty()'), false);
    // An empty placeholder is never written, so undoing back past one has to
    // land clean rather than leaving Save lit with nothing to save.
    run(`addTask(todayKey(), 'Task'); undo();`);
    assert.equal(run('isDirty()'), false);
    run('undo();');
    assert.equal(run('futureKeys().length'), 0);
    assert.equal(run('isDirty()'), false);
});

test('undo history is bounded', () => {
    const run = panel();
    run(`addTask(todayKey(), 'Task');
        for (let i = 0; i < UNDO_HISTORY_LIMIT + 10; i++)
            toggleTask(todayKey(), tasksByDay.get(todayKey())[0].id);`);
    assert.equal(run('undoHistory.length'), run('UNDO_HISTORY_LIMIT'));
});
