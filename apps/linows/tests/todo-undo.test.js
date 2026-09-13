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
    assert.equal(run('dirty'), false);
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

test('successful save prevents undo and redo of saved changes; new edits start fresh', async () => {
    let saved;
    const run = panel(async (tasks) => {
        saved = structuredClone(tasks);
    });
    run(`addTask(todayKey(), 'Keep');`);
    await run('persist()');
    run('clearAll(todayKey());');
    await run('persist()');
    assert.equal(saved.length, 0);
    run('undo(); redo();');
    assert.equal(run('undoHistory.length'), 0);
    assert.equal(run('redoHistory.length'), 0);
    assert.equal(run('tasksByDay.get(todayKey()).length'), 0);
    assert.equal(run('dirty'), false);
    run(`addTask(todayKey(), 'New'); undo();`);
    assert.equal(run('tasksByDay.get(todayKey()).length'), 0);
    assert.equal(run('dirty'), false);
    run('redo();');
    assert.equal(run('tasksByDay.get(todayKey())[0].name'), 'New');
});

test('save completion clears history while edits during save remain dirty', async () => {
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
    assert.equal(run('dirty'), true);
    run('undo(); redo();');
    assert.equal(run('undoHistory.length'), 0);
    assert.equal(run('redoHistory.length'), 0);
    assert.equal(run('dirty'), true);
    assert.equal(run('tasksByDay.get(todayKey()).length'), 2);
});

test('failed save retains undo and dirty state', async () => {
    const run = panel(async () => {
        throw new Error('unavailable');
    });
    run(`addTask(todayKey(), 'Unsaved');`);
    await run('persist()');
    assert.equal(run('dirty'), true);
    run('undo();');
    assert.equal(run('dirty'), false);
});

test('Ctrl/Cmd+Z and Shift+Z undo/redo without taking text or hidden-panel keys', () => {
    const run = panel();
    run(`visible = true;
        const key = { key: 'z', ctrlKey: true, preventDefault() {} };
        addTask(todayKey(), 'Task');
        document.activeElement = { dataset: { todoField: 'rename' } };`);
    assert.equal(run('handleKey(key)'), false);
    run('document.activeElement = searchInput; searchInput.value = "query";');
    assert.equal(run('handleKey(key)'), false);
    run('searchInput.value = "";');
    assert.equal(run('handleKey({ ...key, shiftKey: true })'), false);
    assert.equal(run('handleKey(key)'), true);
    assert.equal(run('dirty'), false);
    run("document.activeElement = { dataset: { todoField: 'rename' } };");
    assert.equal(run('handleKey({ ...key, shiftKey: true })'), false);
    run('document.activeElement = searchInput;');
    assert.equal(run('handleKey({ ...key, shiftKey: true })'), true);
    assert.equal(run('tasksByDay.get(todayKey())[0].name'), 'Task');
    assert.equal(run('dirty'), true);
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
    assert.equal(run('dirty'), false);
    run('redo(); redo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), changed);
    assert.equal(run('dirty'), true);
    run('undo(); undo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), saved);
    assert.equal(run('dirty'), false);
    run('clearAll(todayKey());');
    const deleted = run('JSON.stringify([...tasksByDay])');
    run('undo(); redo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), deleted);
    assert.equal(run('dirty'), true);
    run('undo();');
    assert.equal(run('JSON.stringify([...tasksByDay])'), saved);
});

test('saving after undo clears redo even when undo returned to a clean state', async () => {
    const run = panel();
    run(`addTask(todayKey(), 'Discard'); undo();`);
    assert.equal(run('dirty'), false);
    assert.equal(run('redoHistory.length'), 1);
    await run('persist()');
    run('redo();');
    assert.equal(run('undoHistory.length'), 0);
    assert.equal(run('redoHistory.length'), 0);
    assert.equal(run('tasksByDay.get(todayKey()).length'), 0);
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

test('save completion clears redo created while saving without clearing unsaved edits', async () => {
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
    assert.equal(run('redoHistory.length'), 0);
    assert.equal(run('dirty'), true);
    assert.equal(run('tasksByDay.get(todayKey()).length'), 0);
});

test('date placeholders undo in order and history is bounded', () => {
    const run = panel();
    run('addDate(); undo();');
    assert.equal(run('futureKeys().length'), 0);
    run(`addTask(todayKey(), 'Task');
        for (let i = 0; i < 60; i++) toggleTask(todayKey(), tasksByDay.get(todayKey())[0].id);`);
    assert.equal(run('undoHistory.length'), 50);
});
