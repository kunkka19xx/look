import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const source = readFileSync(new URL('../src/js/screens/settings.js', import.meta.url), 'utf8')
    .replace(/^import\s[\s\S]*?\sfrom\s+['"][^'"]+['"];\r?\n/gm, '')
    .replace(/^export /gm, '');
const defaults = readFileSync(
    new URL('../src/js/theme-defaults.js', import.meta.url),
    'utf8',
).replace(/^export /gm, '');

// Exercise the real Settings event handlers with isolated DOM and IPC boundaries.
async function settings({
    failSave = false,
    failAutostart = false,
    failPath = false,
    savedEntries = {},
} = {}) {
    function element() {
        const children = new Map();
        const listeners = new Map();
        const classes = new Set();
        const attributes = new Map();
        const properties = new Map();
        return {
            value: '',
            checked: false,
            textContent: '',
            dataset: {},
            children: [],
            style: {
                setProperty: (key, value) => properties.set(key, value),
                removeProperty: (key) => properties.delete(key),
                getPropertyValue: (key) => properties.get(key),
            },
            classList: {
                add: (key) => classes.add(key),
                remove: (key) => classes.delete(key),
                contains: (key) => classes.has(key),
                toggle: (key, on) => (on ? classes.add(key) : classes.delete(key)),
            },
            addEventListener: (type, callback) => listeners.set(type, callback),
            fire(type, event = {}) {
                return listeners.get(type)?.({ target: this, ...event });
            },
            querySelector(selector) {
                if (!children.has(selector)) children.set(selector, element());
                return children.get(selector);
            },
            querySelectorAll: () => [],
            appendChild(child) {
                this.children.push(child);
            },
            closest() {
                return this;
            },
            contains: () => false,
            setAttribute: (key, value) => attributes.set(key, value),
            removeAttribute: (key) => attributes.delete(key),
            getAttribute: (key) => attributes.get(key) ?? null,
            hasAttribute: (key) => attributes.has(key),
        };
    }
    const elements = new Map();
    const get = (id) => {
        if (!elements.has(id)) elements.set(id, element());
        return elements.get(id);
    };
    const root = element();
    const rows = ['ui_font_size', 'ui_tint_red'].map((key) => {
        const row = element();
        row.dataset.key = key;
        return row;
    });
    const screen = get('settings-screen');
    screen.querySelectorAll = (selector) => (selector === '.settings-row[data-key]' ? rows : []);
    const originalQuery = screen.querySelector.bind(screen);
    screen.querySelector = (selector) => {
        const key = selector.match(/data-key="([^"]+)"/)?.[1];
        const row = rows.find((row) => row.dataset.key === key);
        if (key)
            return selector.endsWith(' .settings-slider')
                ? row?.querySelector('.settings-slider')
                : row;
        return originalQuery(selector);
    };
    const writes = [];
    const systemChanges = [];
    const events = [];
    const applications = [];
    const banners = [];
    const saved = new Map([
        ['ui_font_size', '14'],
        ['file_scan_extra_roots', '/original'],
        ['file_exclude_paths', '/skip'],
        ['layout', 'split'],
        ...Object.entries(savedEntries),
    ]);
    const noOp = () => {};
    const context = vm.createContext({
        document: {
            documentElement: root,
            getElementById: get,
            querySelector: () => null,
            createElement: element,
            addEventListener: noOp,
            dispatchEvent: (event) => events.push(event),
        },
        CustomEvent: class {
            constructor(type, { detail }) {
                this.type = type;
                this.detail = detail;
            }
        },
        localStorage: { getItem: () => null },
        window: { __TAURI__: { core: { convertFileSrc: (path) => path } } },
        console: { error: noOp },
        setTimeout: () => 0,
        clearTimeout: noOp,
        getConfig: async () => ({ entries: [...saved].map(([key, value]) => ({ key, value })) }),
        setConfig: async (updates) => {
            if (failSave) throw new Error('read-only config');
            writes.push(updates);
            for (const { key, value } of updates) saved.set(key, value);
        },
        forceIndexRefresh: async () => applications.push('index'),
        listFonts: async () => [],
        listCandidateDrives: async () => [],
        pickFolder: async () => '/new,folder',
        pickImage: async () => '/wallpaper.png',
        getAutostart: async () => false,
        getCliPath: async () => false,
        setAutostart: async (on) => {
            systemChanges.push(['autostart', on]);
            if (failAutostart) throw new Error('autostart permission denied');
        },
        setCliPath: async (on) => {
            systemChanges.push(['path', on]);
            if (failPath) throw new Error('PATH permission denied');
        },
        banner: { show: (text) => banners.push(text) },
        sourceblocks: { reload: async () => applications.push('sources') },
        platform: new Proxy(
            {
                getBlurStyles: () => [{ value: 'high_contrast', label: 'High Contrast' }],
                isWindows: () => true,
                floatingSupported: () => true,
            },
            { get: (target, key) => target[key] ?? noOp },
        ),
        layout: new Proxy(
            { parseLayout: (value) => value || 'split' },
            {
                get: (target, key) => target[key] ?? noOp,
            },
        ),
        superactions: new Proxy({}, { get: () => noOp }),
        shortcutrecorder: new Proxy(
            { pendingUpdates: () => ({ launcher_hotkey: 'ctrl+space' }) },
            {
                get: (target, key) => target[key] ?? noOp,
            },
        ),
    });
    vm.runInContext(
        defaults +
            '\n' +
            source +
            '\nglobalThis.api = { init, enter, exit, restoreOnStartup, setOnConfigReload };',
        context,
    );
    context.api.init(noOp);
    await context.api.enter(element(), element());
    get('settings-layout').querySelector('.settings-segment-active').dataset.value =
        saved.get('layout');
    context.api.setOnConfigReload(() => applications.push('config'));
    return {
        get,
        rows,
        writes,
        saved,
        systemChanges,
        events,
        root,
        context,
        applications,
        banners,
    };
}

test('editing controls and folder lists never writes config or changes startup integration', async () => {
    const { get, rows, writes, systemChanges, events, root } = await settings();
    const slider = rows[0].querySelector('.settings-slider');
    slider.value = '20';
    await slider.fire('input');
    await slider.fire('change');
    assert.equal(root.style.getPropertyValue('--font-size'), '20px');
    for (const id of ['settings-launch-login', 'settings-add-to-path', 'settings-disable-gpu']) {
        get(id).checked = true;
        await get(id).fire('change');
    }
    await get('settings-add-scan-dir').fire('click');
    await get('settings-add-scan-dir').fire('click');
    await get('settings-add-skip-dir').fire('click');
    await get('settings-choose-bg').fire('click');
    for (const [id, value] of [
        ['settings-theme', 'kindle'],
        ['settings-blur-style', 'balanced'],
        ['settings-bg-layout', 'stretch'],
        ['settings-log-level', 'debug'],
    ]) {
        const item = { dataset: { value }, textContent: value, classList: { add() {} } };
        await get(id)
            .querySelector('.settings-dropdown-menu')
            .fire('click', {
                target: { closest: () => item },
            });
    }
    const item = { dataset: { value: 'compact' }, classList: { contains: () => false } };
    await get('settings-layout').fire('click', { target: { closest: () => item } });
    assert.ok(
        events.some(
            (event) => event.type === 'look:layout-preview' && event.detail.value === 'compact',
        ),
    );
    assert.equal(writes.length, 0);
    assert.deepEqual(systemChanges, []);
});

test('Save Config writes staged values once, including directory lists and rendering switches', async () => {
    const { get, writes, saved, systemChanges } = await settings();
    await get('settings-add-scan-dir').fire('click');
    await get('settings-add-scan-dir').fire('click');
    for (const id of [
        'settings-disable-gpu',
        'settings-compositor-blur',
        'settings-disable-blur',
        'settings-launch-login',
        'settings-add-to-path',
    ]) {
        get(id).checked = true;
        await get(id).fire('change');
    }
    await get('settings-save-btn').fire('click');
    assert.equal(writes.length, 1);
    assert.equal(saved.get('file_scan_extra_roots'), '/original,/new\\,folder');
    assert.equal(saved.get('file_exclude_paths'), '/skip');
    for (const key of ['disable_gpu_compositing', 'ui_compositor_blur', 'disable_blur_effect']) {
        assert.equal(saved.get(key), 'true');
    }
    assert.equal(saved.get('launcher_hotkey'), 'ctrl+space');
    assert.deepEqual(systemChanges, [
        ['autostart', true],
        ['path', true],
    ]);
});

test('leaving Settings discards unsaved edits and restores the saved preview', async () => {
    const { get, rows, writes, saved, root, context } = await settings();
    const slider = rows[0].querySelector('.settings-slider');
    slider.value = '24';
    await slider.fire('input');
    await slider.fire('change');
    await get('settings-add-scan-dir').fire('click');
    await get('settings-choose-bg').fire('click');
    get('settings-disable-blur').checked = true;
    await get('settings-disable-blur').fire('change');
    context.api.exit(get('search-content'), get('search-bar'));
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(root.style.getPropertyValue('--font-size'), '14px');
    assert.equal(root.style.getPropertyValue('--bg-image'), undefined);
    assert.equal(root.hasAttribute('data-disable-blur'), false);
    assert.equal(writes.length, 0);
    assert.equal(saved.get('file_scan_extra_roots'), '/original');
    await context.api.enter(get('search-content'), get('search-bar'));
    await get('settings-save-btn').fire('click');
    assert.equal(saved.get('file_scan_extra_roots'), '/original');
    assert.equal(saved.get('ui_font_size'), '14');
});

test('failed Save Config keeps staged edits for retry and does not apply startup changes', async () => {
    const { get, writes, systemChanges, context } = await settings({ failSave: true });
    await get('settings-add-scan-dir').fire('click');
    get('settings-launch-login').checked = true;
    await get('settings-launch-login').fire('change');
    await get('settings-save-btn').fire('click');
    assert.equal(get('settings-save-msg').textContent, 'Save failed');
    assert.equal(writes.length, 0);
    assert.deepEqual(systemChanges, []);
    context.setConfig = async (updates) => writes.push(updates);
    await get('settings-save-btn').fire('click');
    assert.equal(writes.length, 1);
    assert.equal(
        writes[0].find(({ key }) => key === 'file_scan_extra_roots').value,
        '/original,/new\\,folder',
    );
    assert.deepEqual(systemChanges, [
        ['autostart', true],
        ['path', false],
    ]);
});

test('layout previews resize through the session override and restore the earlier session layout', () => {
    const app = readFileSync(new URL('../src/js/app.js', import.meta.url), 'utf8');
    const logic = app.slice(
        app.indexOf('    let compactApplied = null;'),
        app.indexOf('    // The empty-state control strip'),
    );
    const resizes = [];
    let compact = false;
    const noOp = () => {};
    const consumer = { setCompact: noOp };
    const context = vm.createContext({
        layout: {
            LAYOUT_COMPACT: 'compact',
            LAYOUT_SPLIT: 'split',
            parseLayout: (value) => value,
            setCompact: (value) => {
                compact = value;
            },
        },
        runningApps: consumer,
        superactions: consumer,
        actionmenu: consumer,
        syncControlStrip: noOp,
        applyAiLayoutMode: noOp,
        document: { dispatchEvent: noOp },
        CustomEvent: class {},
        console,
        applyLayout: async (override) => {
            resizes.push(override);
        },
    });
    vm.runInContext(
        logic +
            '\nglobalThis.api = { applyLayoutSetting, toggleSessionLayout, preview(value) { settingsLayout = value; applyEffectiveLayout(); } };',
        context,
    );
    context.api.applyLayoutSetting('split');
    context.api.toggleSessionLayout();
    assert.equal(compact, true);
    // A reload after an unrelated save keeps Ctrl+Shift+C's session override.
    context.api.applyLayoutSetting('split');
    assert.equal(compact, true);
    context.api.preview('split');
    assert.equal(compact, false);
    assert.equal(resizes.at(-1), 'split');
    context.api.preview(null);
    assert.equal(compact, true);
    assert.equal(resizes.at(-1), 'compact');
    context.api.preview('compact');
    context.api.applyLayoutSetting('compact', { fromSettings: true });
    context.api.preview(null);
    assert.equal(compact, true);
    assert.equal(resizes.at(-1), null);
});

test('discarding a preset preview restores Custom base CSS tokens and saved tint', async () => {
    const { get, root, context, writes } = await settings({
        savedEntries: {
            ui_theme: 'custom',
            ui_tint_red: '0.2',
            ui_tint_green: '0.3',
            ui_tint_blue: '0.4',
        },
    });
    await context.api.restoreOnStartup();
    assert.equal(root.getAttribute('data-theme'), 'custom');
    const tint = root.style.getPropertyValue('--bg-tint');
    const item = { dataset: { value: 'kindle' } };
    await get('settings-theme')
        .querySelector('.settings-dropdown-menu')
        .fire('click', {
            target: { closest: () => item },
        });
    assert.equal(root.getAttribute('data-theme'), 'kindle');
    context.api.exit(get('search-content'), get('search-bar'));
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(root.getAttribute('data-theme'), 'custom');
    assert.equal(root.style.getPropertyValue('--bg-tint'), tint);
    assert.equal(writes.length, 0);
});

test('unrelated saves leave the session layout override intact', async () => {
    const { get, events, saved } = await settings();
    get('settings-disable-gpu').checked = true;
    await get('settings-disable-gpu').fire('change');
    await get('settings-save-btn').fire('click');
    assert.equal(saved.get('layout'), 'split');
    assert.ok(!events.some(({ type }) => type === 'look:layout-changed'));
    const item = { dataset: { value: 'compact' }, classList: { contains: () => false } };
    await get('settings-layout').fire('click', { target: { closest: () => item } });
    get('settings-layout').querySelector('.settings-segment-active').dataset.value = 'compact';
    await get('settings-save-btn').fire('click');
    assert.equal(saved.get('layout'), 'compact');
    assert.ok(
        events.some(
            ({ type, detail }) => type === 'look:layout-changed' && detail.value === 'compact',
        ),
    );
});

for (const failure of ['failAutostart', 'failPath']) {
    test(`${failure} does not block applying saved config or the other integration`, async () => {
        const { get, saved, writes, applications, systemChanges, banners, events } = await settings(
            { [failure]: true },
        );
        get('settings-scan-depth').value = '8';
        await get('settings-scan-depth').fire('change');
        const item = { dataset: { value: 'compact' }, classList: { contains: () => false } };
        await get('settings-layout').fire('click', { target: { closest: () => item } });
        get('settings-layout').querySelector('.settings-segment-active').dataset.value = 'compact';
        await get('settings-save-btn').fire('click');
        assert.equal(writes.length, 1);
        assert.equal(saved.get('file_scan_depth'), '8');
        assert.deepEqual(applications, ['sources', 'index', 'config']);
        assert.deepEqual(systemChanges, [
            ['autostart', false],
            ['path', false],
        ]);
        assert.ok(events.some(({ type }) => type === 'look:layout-changed'));
        assert.equal(get('settings-save-msg').textContent, 'Saved; startup update failed');
        assert.match(
            banners.at(-1),
            failure === 'failAutostart'
                ? /Launch at login.*autostart permission denied/
                : /PATH.*PATH permission denied/,
        );
    });
}
