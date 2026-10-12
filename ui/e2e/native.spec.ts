/** Proves desktop UI integration with official Tauri JavaScript APIs and mocked IPC only. */
import { appendFile, mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import type { FixtureEngine } from '../scripts/engine';
import type { Session } from '../src/gen/Session';
import type { EventRecord } from '../src/gen/EventRecord';
import { test, expect } from './fixtures';
type Call = { command: string; args: Record<string, unknown> };
type MockNative = {
  calls: Call[];
  ready: boolean;
  clipboard: string;
  saveResult: string | null;
  pickResult: string | null;
  holdPaths: boolean;
  rejectPath: (() => void) | null;
  emit: (action: string) => void;
};
declare global {
  interface Window {
    mockNative: MockNative;
  }
}
const mock = () => window.mockNative;
const primary = (page: Page) => page.getByRole('region', { name: 'Primary session', exact: true });
const right = (page: Page) => page.getByRole('region', { name: 'Right session', exact: true });
const prompt = (region: ReturnType<typeof primary>) =>
  region.getByLabel('Message', { exact: true });
async function install(page: Page) {
  await page.addInitScript(() => {
    let serial = 0,
      autostart = false;
    const callbacks = new Map<number, (event: unknown) => void>();
    const listeners = new Map<number, { event: string; handler: number }>();
    const state: MockNative = {
      calls: [],
      ready: false,
      clipboard: '',
      saveResult: '/synthetic/export.md',
      pickResult: null,
      holdPaths: false,
      rejectPath: null,
      emit(action) {
        for (const [id, listener] of listeners)
          if (listener.event === 'native-action')
            callbacks.get(listener.handler)?.({ event: listener.event, id, payload: action });
      },
    };
    Object.assign(window, {
      isTauri: true,
      mockNative: state,
      __TAURI_EVENT_PLUGIN_INTERNALS__: {
        unregisterListener: (_event: string, id: number) => listeners.delete(id),
      },
      __TAURI_INTERNALS__: {
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
        transformCallback: (callback: (event: unknown) => void) => {
          const id = ++serial;
          callbacks.set(id, callback);
          return id;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        async invoke(command: string, args: Record<string, unknown> = {}) {
          state.calls.push({ command, args });
          switch (command) {
            case 'plugin:event|listen': {
              const id = ++serial;
              listeners.set(id, { event: String(args.event), handler: Number(args.handler) });
              if (args.event === 'native-action') state.ready = true;
              return id;
            }
            case 'plugin:event|unlisten':
              return;
            case 'app_info':
              return { os: 'macos', version: 'mocked-0.5.0' };
            case 'plugin:notification|is_permission_granted':
              return true;
            case 'plugin:autostart|is_enabled':
              return autostart;
            case 'plugin:autostart|enable':
              autostart = true;
              return;
            case 'plugin:autostart|disable':
              autostart = false;
              return;
            case 'plugin:clipboard-manager|read_text':
              return state.clipboard;
            case 'plugin:clipboard-manager|write_text':
              state.clipboard = String(args.text);
              return;
            case 'save_file':
              return state.saveResult;
            case 'pick_folder':
              return state.pickResult;
            case 'restart_engine':
            case 'plugin:webview|set_webview_zoom':
            case 'plugin:opener|open_url':
              return;
            case 'plugin:opener|open_path':
            case 'plugin:opener|reveal_item_in_dir':
              if (state.holdPaths)
                await new Promise<void>((_resolve, reject) => {
                  state.rejectPath = () => reject(new Error('Late native folder failure'));
                });
              return;
            case 'plugin:window|request_user_attention':
            case 'plugin:window|start_dragging':
              return;
            default:
              throw new Error(`Unmocked native command: ${command}`);
          }
        },
      },
    });
  });
}
async function calls(page: Page, command: string) {
  return page.evaluate(
    (command) => window.mockNative.calls.filter((call) => call.command === command),
    command,
  );
}
async function emit(page: Page, action: string) {
  await page.evaluate((action) => window.mockNative.emit(action), action);
}
async function create(engine: FixtureEngine, title: string) {
  const response = await engine.call('POST', '/api/sessions', {
    title,
    cwd: engine.sandbox.project,
  });
  expect(response.status).toBe(200);
  return JSON.parse(response.text) as Session;
}
async function open(page: Page, engine: FixtureEngine, id: string) {
  await install(page);
  await page.goto(`${engine.url}/#${id}`);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await expect.poll(() => page.evaluate(mock).then((value) => value.ready)).toBe(true);
  await expect(page.locator('html')).toHaveAttribute('data-native-os', 'macos');
}
async function section(page: Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name, exact: true })
    .click();
}
async function shot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice10');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `mocked-native-${name}.png`), fullPage: true });
}

test('mocked native menus navigate the active right pane, preserve session drafts and create from primary', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Native left'),
    two = await create(engine, 'Native right');
  await open(page, engine, one.id);
  await prompt(primary(page)).fill('Unsubmitted left draft');
  await emit(page, 'split');
  await page.getByRole('menuitem', { name: 'Native right', exact: true }).click();
  await prompt(right(page)).fill('Unsubmitted right draft');
  await prompt(right(page)).focus();
  await emit(page, 'next_session');
  await expect(
    right(page).getByRole('heading', { name: 'Native left', exact: true }),
  ).toBeVisible();
  await expect(prompt(right(page))).toHaveValue('Unsubmitted left draft');
  await expect(prompt(primary(page))).toHaveValue('Unsubmitted right draft');
  await emit(page, 'prev_session');
  await expect(
    right(page).getByRole('heading', { name: 'Native right', exact: true }),
  ).toBeVisible();
  await expect(prompt(right(page))).toHaveValue('Unsubmitted right draft');
  await expect(prompt(primary(page))).toHaveValue('Unsubmitted left draft');
  await emit(page, 'find');
  await expect(page.getByRole('textbox', { name: 'Filter sessions' })).toBeFocused();
  await prompt(right(page)).focus();
  await emit(page, 'new');
  const dialog = page.getByRole('dialog', { name: 'New session', exact: true });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(prompt(primary(page))).toHaveValue('Unsubmitted left draft');
  await expect(prompt(right(page))).toHaveValue('Unsubmitted right draft');
  await emit(page, 'shortcuts');
  await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeVisible();
  await page
    .getByRole('dialog', { name: 'Keyboard shortcuts' })
    .getByRole('button', { name: 'Close', exact: true })
    .click();
  await emit(page, `session:${two.id}`);
  await expect(
    primary(page).getByRole('heading', { name: 'Native right', exact: true }),
  ).toBeVisible();
  await expect(prompt(primary(page))).toHaveValue('Unsubmitted right draft');
  await emit(page, 'not_a_native_action');
  await expect(prompt(primary(page))).toHaveValue('Unsubmitted right draft');
  await shot(page, engine, 'menu-pane-ownership');
  const response = await engine.call('GET', '/api/state');
  expect(response.status).toBe(200);
  expect(JSON.parse(response.text).sessions).toHaveLength(2);
});

test('mocked native zoom uses one webview in split view and appearance and autostart use desktop APIs', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Zoom left'),
    two = await create(engine, 'Zoom right');
  await open(page, engine, one.id);
  await emit(page, 'split');
  await page.getByRole('menuitem', { name: two.title, exact: true }).click();
  await emit(page, 'zoom_in');
  await expect
    .poll(async () => (await calls(page, 'plugin:webview|set_webview_zoom')).at(-1)?.args)
    .toEqual({ label: 'main', value: 1.1 });
  await emit(page, 'zoom_out');
  await expect
    .poll(async () => (await calls(page, 'plugin:webview|set_webview_zoom')).at(-1)?.args.value)
    .toBe(1);
  await emit(page, 'settings');
  await expect(page.getByRole('dialog', { name: 'General', exact: true })).toBeVisible();
  const autostart = page.getByRole('checkbox', { name: 'Open at login', exact: true });
  await expect(autostart).not.toBeChecked();
  await autostart.check();
  await expect(autostart).toBeChecked();
  expect(await calls(page, 'plugin:autostart|enable')).toHaveLength(1);
  await autostart.uncheck();
  await expect(autostart).not.toBeChecked();
  expect(await calls(page, 'plugin:autostart|disable')).toHaveLength(1);
  await section(page, 'Appearance');
  await page.getByRole('button', { name: 'Larger interface' }).click();
  await expect(page.getByRole('button', { name: 'Actual size' })).toHaveText('110%');
  await expect
    .poll(async () => (await calls(page, 'plugin:webview|set_webview_zoom')).at(-1)?.args.value)
    .toBe(1.1);
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).zoom)).toBe('1');
  expect(
    await page.evaluate(() => document.documentElement.style.getPropertyValue('--native-top')),
  ).toBe(`${52 / 1.1}px`);
  await shot(page, engine, 'appearance-zoom');
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.reload();
  await expect(right(page)).toBeVisible();
  await expect
    .poll(async () => (await calls(page, 'plugin:webview|set_webview_zoom')).at(-1)?.args)
    .toEqual({ label: 'main', value: 1.1 });
  await emit(page, 'zoom_reset');
  await expect
    .poll(async () => (await calls(page, 'plugin:webview|set_webview_zoom')).at(-1)?.args.value)
    .toBe(1);
});
test('mocked native export clipboard file links and engine controls preserve local UI ownership', async ({
  page,
  engine,
}) => {
  const session = await create(engine, 'Native / export');
  await writeFile(join(engine.sandbox.project, 'native note.md'), 'first\nselected\nlast\n');
  const text =
    '```ts\nconst native = "clipboard";\n```\n\n[external](https://example.invalid/native) [file](file://' +
    engine.sandbox.project +
    '/native%20note.md#L2)';
  const events: EventRecord[] = [
    { t: 'user', ts: 1, kind: 'message', text: 'Durable native prompt' },
    { t: 'assistant', ts: 2, turn: 1, step: 1, text },
    {
      t: 'turn_end',
      ts: 3,
      turn: 1,
      slot: 'one@example.com',
      status: 'SUCCESS',
      secs: 1,
      usage: { input_tokens: 1, output_tokens: 1, thinking_tokens: 0, total_tokens: 2, context: 0 },
    },
  ];
  const port = engine.port;
  await engine.stop();
  await appendFile(
    join(engine.sandbox.data, 'sessions', session.id, 'events.jsonl'),
    events.map((event) => JSON.stringify(event)).join('\n') + '\n',
  );
  await engine.start(port);
  await open(page, engine, session.id);
  await prompt(primary(page)).fill('Private unsent draft');
  await emit(page, 'export');
  await expect.poll(async () => (await calls(page, 'save_file')).length).toBe(1);
  const saved = (await calls(page, 'save_file'))[0]!;
  expect(saved.args.name).toBe('Native-export.md');
  expect(saved.args.contents).toContain('## you\n\nDurable native prompt');
  expect(saved.args.contents).toContain(text);
  expect(saved.args.contents).not.toContain('Private unsent draft');
  await expect(prompt(primary(page))).toHaveValue('Private unsent draft');
  await page.evaluate(() => {
    window.mockNative.saveResult = null;
  });
  await emit(page, 'export');
  await expect.poll(async () => (await calls(page, 'save_file')).length).toBe(2);
  await expect(prompt(primary(page))).toHaveValue('Private unsent draft');
  const reply = page.getByLabel('Agent reply');
  await reply.getByRole('button', { name: 'Copy code' }).click();
  expect((await calls(page, 'plugin:clipboard-manager|write_text')).at(-1)?.args.text).toBe(
    'const native = "clipboard";',
  );
  await reply.getByRole('button', { name: 'Copy message' }).click();
  expect((await calls(page, 'plugin:clipboard-manager|write_text')).at(-1)?.args.text).toBe(text);
  const before = page.url();
  await reply.getByRole('link', { name: 'external', exact: true }).click();
  await expect.poll(async () => (await calls(page, 'plugin:opener|open_url')).length).toBe(1);
  expect((await calls(page, 'plugin:opener|open_url'))[0]?.args.url).toBe(
    'https://example.invalid/native',
  );
  expect(page.context().pages()).toHaveLength(1);
  expect(page.url()).toBe(before);
  await reply.getByRole('link', { name: 'file', exact: true }).click();
  const viewer = page.getByRole('dialog', { name: 'native note.md', exact: true });
  await expect(viewer.locator('.line.highlight .text')).toHaveText('selected');
  await viewer.getByRole('button', { name: 'Copy', exact: true }).click();
  expect((await calls(page, 'plugin:clipboard-manager|write_text')).at(-1)?.args.text).toBe(
    'first\nselected\nlast\n',
  );
  await viewer.getByRole('button', { name: 'Close file' }).click();
  expect(await calls(page, 'plugin:opener|open_url')).toHaveLength(1);
  let shutdown = 0;
  page.on('request', (request) => {
    if (new URL(request.url()).pathname === '/api/shutdown') shutdown++;
  });
  await emit(page, 'settings');
  await section(page, 'Engine');
  await expect(page.getByText('Desktop mocked-0.5.0 · macos')).toBeVisible();
  await page.getByRole('button', { name: 'Data directory', exact: true }).click();
  expect((await calls(page, 'plugin:opener|open_path')).at(-1)?.args.path).toBe(
    engine.sandbox.data,
  );
  await page.getByRole('button', { name: 'Restart engine', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Restart the engine?' })
    .getByRole('button', { name: 'Restart', exact: true })
    .click();
  await expect(page.getByText('Engine restarted.')).toBeVisible();
  expect(await calls(page, 'restart_engine')).toHaveLength(1);
  expect(shutdown).toBe(0);
  await shot(page, engine, 'capabilities-and-engine');
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(prompt(primary(page))).toHaveValue('Private unsent draft');
});
test('mocked native folder failures stay with their original session and disposed panel', async ({
  page,
  engine,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const one = await create(engine, 'Folder owner'),
    two = await create(engine, 'New folder owner');
  await open(page, engine, one.id);
  await emit(page, 'panel');
  await page.getByRole('tab', { name: 'Files', exact: true }).click();
  for (const phase of ['session', 'disposed']) {
    await page.evaluate(() => {
      const bridge = window.mockNative;
      bridge.holdPaths = true;
      bridge.rejectPath = null;
    });
    await page
      .getByRole('button', {
        name: phase === 'session' ? 'Open folder' : 'Reveal folder',
        exact: true,
      })
      .click();
    await expect.poll(() => page.evaluate(() => !!window.mockNative.rejectPath)).toBe(true);
    if (phase === 'session') {
      await emit(page, `session:${two.id}`);
      await expect(
        primary(page).getByRole('heading', { name: two.title, exact: true }),
      ).toBeVisible();
    } else {
      await page.getByRole('button', { name: 'Close side panel' }).click();
      await emit(page, 'panel');
    }
    await expect(page.getByLabel('Folder contents')).toBeVisible();
    await page.evaluate(async () => {
      window.mockNative.rejectPath?.();
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    await expect(page.getByRole('button', { name: 'Open folder', exact: true })).toBeVisible();
    await expect(page.getByText('Late native folder failure', { exact: false })).toHaveCount(0);
  }
  await emit(page, 'settings');
  await section(page, 'Engine');
  await page.evaluate(() => {
    window.mockNative.rejectPath = null;
  });
  await page.getByRole('button', { name: 'Data directory', exact: true }).click();
  await expect.poll(() => page.evaluate(() => !!window.mockNative.rejectPath)).toBe(true);
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.evaluate(async () => {
    window.mockNative.rejectPath?.();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  await expect(page.getByText('Late native folder failure', { exact: false })).toHaveCount(0);
  expect(errors).toEqual([]);
  await shot(page, engine, 'folder-error-ownership');
});

test('mocked native new session uses the system folder picker and no model choice', async ({
  page,
  engine,
}) => {
  const session = await create(engine, 'Picker origin');
  await open(page, engine, session.id);
  await page
    .getByRole('button', { name: /^New session/ })
    .first()
    .click();
  const dialog = page.getByRole('dialog', { name: /^New session/ });
  const folder = dialog.getByRole('textbox', { name: 'Project folder' });
  await expect(dialog.getByRole('combobox', { name: /model|effort/i })).toHaveCount(0);
  await expect(dialog.getByRole('button', { name: /\/$/ })).toHaveCount(0);
  // Cancelling the picker keeps the typed folder.
  await dialog.getByRole('button', { name: 'Choose folder…' }).click();
  await expect.poll(async () => (await calls(page, 'pick_folder')).length).toBe(1);
  await expect(folder).toHaveValue(engine.sandbox.project);
  const picked = join(engine.sandbox.project, 'picked');
  await mkdir(picked, { recursive: true });
  await page.evaluate((path) => (window.mockNative.pickResult = path), picked);
  await dialog.getByRole('button', { name: 'Choose folder…' }).click();
  await expect(folder).toHaveValue(picked);
  expect((await calls(page, 'pick_folder'))[1]!.args).toEqual({ start: engine.sandbox.project });
  const created = page.waitForResponse(
    (r) => new URL(r.url()).pathname === '/api/sessions' && r.request().method() === 'POST',
  );
  await dialog.getByRole('button', { name: 'Create session' }).click();
  expect((await created).request().postDataJSON()).toMatchObject({ cwd: picked });
});
