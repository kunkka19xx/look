import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

// Isolate the widget's DOM and IPC boundaries without loading the desktop shell.
const source = readFileSync(new URL('../src/js/screens/update_widget.js', import.meta.url), 'utf8')
    .replace(/^import\s[\s\S]*?\sfrom\s+['"][^'"]+['"];\r?\n/gm, '')
    .replace('export async function', 'async function');

async function widget({ installMethod = 'nsis', os = 'windows', failure, isDev = false, check = true, latest = '0.6.11', dismissedVersion } = {}) {
    const calls = [];
    const copied = [];
    const listeners = new Map();
    const localStorage = {
        values: dismissedVersion ? new Map([['look.update.dismissedVersion', dismissedVersion]]) : new Map(),
        getItem(key) { return this.values.get(key) ?? null; },
        setItem(key, value) { this.values.set(key, String(value)); },
    };
    const container = {
        innerHTML: '',
        classList: { add() {} },
        querySelector(selector) {
            return this.innerHTML.includes(selector.slice(1, -1))
                ? { addEventListener: (_, callback) => listeners.set(selector, callback) }
                : null;
        },
    };
    const context = vm.createContext({
        platform: { os: () => os },
        getInstallMethod: async () => installMethod,
        getLookappVersion: async () => '0.6.10',
        isDevBuild: async () => isDev,
        startWindowsUpdate: async version => {
            calls.push(version);
            if (failure) throw new Error(failure);
        },
        fetch: async () => ({ ok: true, json: async () => ({
            tag_name: `v${latest}`, html_url: 'https://github.com/kunkka19xx/look/releases/tag/v0.6.11',
        }) }),
        localStorage,
        navigator: { clipboard: { writeText: async text => { copied.push(text); } } },
        AbortController, setTimeout, clearTimeout,
    });
    vm.runInContext(source + '\nglobalThis.mount = mountUpdateWidget;', context);
    await context.mount(container);
    const click = action => {
        const callback = listeners.get(`[data-action="${action}"]`);
        assert.ok(callback, `Missing ${action} button`);
        return callback();
    };
    if (check) await click('check');
    return { container, click, calls, copied };
}

test('Check then Update successfully hands the new version to the native updater', async () => {
    const { container, click, calls } = await widget();
    assert.match(container.innerHTML, /data-action="update"[^>]*>Update<\/button>\s*<button[^>]*data-action="notes">Release Notes/);
    await click('update');
    assert.deepEqual(calls, ['0.6.11']);
    // IPC returning success means handoff only; this does not prove installation or restart.
    assert.match(container.innerHTML, /Downloading update/);
});

test('native updater rejection is shown in the widget', async () => {
    const { container, click } = await widget({ failure: 'Failed to spawn update helper' });
    await click('update');
    assert.match(container.innerHTML, /Failed to spawn update helper/);
});

for (const options of [{ installMethod: 'unknown' }, { installMethod: 'scoop' }, { os: 'linux' }, { isDev: true }]) {
    test(`Update remains unavailable for ${JSON.stringify(options)}`, async () => {
        const { container, calls } = await widget(options);
        assert.ok(!container.innerHTML.includes('data-action="update"'));
        assert.match(container.innerHTML, /data-action="notes"/);
        assert.deepEqual(calls, []);
    });
}

test('opening the widget never updates until the user checks and presses Update', async () => {
    const { calls, container, click } = await widget({ check: false });
    assert.deepEqual(calls, []);
    assert.ok(!container.innerHTML.includes('data-action="update"'));
    await click('check');
    assert.deepEqual(calls, []);
    await click('update');
    assert.deepEqual(calls, ['0.6.11']);
});

test('a manual check surfaces a previously dismissed release', async () => {
    const { calls, click } = await widget({ dismissedVersion: '0.6.11' });
    await click('update');
    assert.deepEqual(calls, ['0.6.11']);
});

test('Scoop installs get a copyable upgrade command instead of Update', async () => {
    const { container, click, copied } = await widget({ installMethod: 'scoop' });
    assert.match(container.innerHTML, /scoop update look/);
    await click('copy-scoop');
    assert.deepEqual(copied, ['scoop update look']);
    assert.match(container.innerHTML, /Command copied/);
});
