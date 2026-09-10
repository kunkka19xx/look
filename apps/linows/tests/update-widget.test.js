import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

// Isolate the widget's DOM and IPC boundaries without loading the desktop shell.
const source = readFileSync(new URL('../src/js/screens/update_widget.js', import.meta.url), 'utf8')
    .replace(/^import\s[\s\S]*?\sfrom\s+['"][^'"]+['"];\r?\n/gm, '')
    .replace('export async function', 'async function');

async function widget({ enabled = false, installMethod = 'nsis', os = 'windows', failure, isDev = false, check = true, latest = '0.6.11' } = {}) {
    const calls = [];
    const listeners = new Map();
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
        autoUpdateEnabled: async () => enabled,
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
    return { container, click, calls, remount: () => context.mount(container) };
}

test('Check then Update successfully hands the new version to the native updater', async () => {
    const { container, click, calls } = await widget();
    assert.match(container.innerHTML, /data-action="update"[^>]*>Update<\/button>\s*<button[^>]*data-action="notes">Notes/);
    await click('update');
    assert.deepEqual(calls, ['0.6.11']);
    // IPC returning success means handoff only; this does not prove installation or restart.
    assert.match(container.innerHTML, /Preparing update/);
});

test('native updater rejection is shown in the widget', async () => {
    const { container, click } = await widget({ failure: 'Failed to spawn update helper' });
    await click('update');
    assert.match(container.innerHTML, /Failed to spawn update helper/);
});

for (const options of [{ installMethod: 'unknown' }, { os: 'linux' }, { isDev: true }]) {
    test(`Update remains unavailable for ${JSON.stringify(options)}`, async () => {
        const { container, calls } = await widget(options);
        assert.ok(!container.innerHTML.includes('data-action="update"'));
        assert.match(container.innerHTML, /data-action="notes"/);
        assert.deepEqual(calls, []);
    });
}

test('Scoop manual Update works with automatic updates disabled', async () => {
    const { click, calls } = await widget({ installMethod: 'scoop', enabled: false });
    await click('update');
    assert.deepEqual(calls, ['0.6.11']);
});

for (const installMethod of ['nsis', 'scoop']) {
    test(`${installMethod} automatically updates once on startup when enabled`, async () => {
        const { calls, remount, click } = await widget({ enabled: true, installMethod, check: false });
        assert.deepEqual(calls, ['0.6.11']);
        await remount();
        await click('update');
        assert.deepEqual(calls, ['0.6.11']);
    });
}

test('disabled startup update does nothing until the user checks', async () => {
    const { calls, container, click } = await widget({ enabled: false, check: false });
    assert.deepEqual(calls, []);
    assert.ok(!container.innerHTML.includes('data-action="update"'));
    await click('check');
    assert.deepEqual(calls, []);
    await click('update');
    assert.deepEqual(calls, ['0.6.11']);
});

test('startup does not install when already on the latest release', async () => {
    const { calls } = await widget({ enabled: true, check: false, latest: '0.6.10' });
    assert.deepEqual(calls, []);
});

test('Scoop launch errors tell the user to check Scoop', async () => {
    const { container, click } = await widget({ installMethod: 'scoop', failure: 'Check your Scoop.' });
    await click('update');
    assert.match(container.innerHTML, /Check your Scoop/);
});
