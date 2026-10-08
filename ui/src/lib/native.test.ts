/** Proves native capability routing, link safety and asynchronous teardown without OS calls. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const bridge = vi.hoisted(() => ({
  isTauri: vi.fn<() => boolean>(),
  invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  listen:
    vi.fn<
      (event: string, callback: (event: { payload: unknown }) => void) => Promise<() => void>
    >(),
  stop: vi.fn(),
  setZoom: vi.fn<(zoom: number) => Promise<void>>(),
  attention: vi.fn<(kind: string) => Promise<void>>(),
  startDragging: vi.fn<() => Promise<void>>(),
  openUrl: vi.fn<(url: string) => Promise<void>>(),
  openPath: vi.fn<(path: string) => Promise<void>>(),
  reveal: vi.fn<(path: string) => Promise<void>>(),
  readText: vi.fn<() => Promise<string>>(),
  writeText: vi.fn<(text: string) => Promise<void>>(),
  enable: vi.fn<() => Promise<void>>(),
  disable: vi.fn<() => Promise<void>>(),
  isEnabled: vi.fn<() => Promise<boolean>>(),
  granted: vi.fn<() => Promise<boolean>>(),
  request: vi.fn<() => Promise<NotificationPermission>>(),
  notify: vi.fn<(options: { title: string; body: string }) => void>(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: bridge.invoke, isTauri: bridge.isTauri }));
vi.mock('@tauri-apps/api/event', () => ({ listen: bridge.listen }));
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ setZoom: bridge.setZoom }),
}));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    requestUserAttention: bridge.attention,
    startDragging: bridge.startDragging,
  }),
  UserAttentionType: { Informational: 'informational' },
}));
vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: bridge.openUrl,
  openPath: bridge.openPath,
  revealItemInDir: bridge.reveal,
}));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  readText: bridge.readText,
  writeText: bridge.writeText,
}));
vi.mock('@tauri-apps/plugin-autostart', () => ({
  enable: bridge.enable,
  disable: bridge.disable,
  isEnabled: bridge.isEnabled,
}));
vi.mock('@tauri-apps/plugin-notification', () => ({
  isPermissionGranted: bridge.granted,
  requestPermission: bridge.request,
  sendNotification: bridge.notify,
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}
type Anchor = {
  href: string;
  target: string;
  download: string;
  hasAttribute: (name: string) => boolean;
};
class LinkTarget {
  constructor(private anchor: Anchor | null) {}
  closest() {
    return this.anchor;
  }
}
async function setup(native = true) {
  bridge.isTauri.mockReturnValue(native);
  const handlers = new Map<string, (event: MouseEvent) => void>();
  const style = { setProperty: vi.fn() };
  const dataset: Record<string, string> = {};
  const browserRead = vi.fn(async () => 'browser clipboard');
  const browserWrite = vi.fn<(text: string) => Promise<void>>(async () => {});
  const open = vi.fn();
  vi.stubGlobal('window', { open });
  vi.stubGlobal('navigator', { clipboard: { readText: browserRead, writeText: browserWrite } });
  vi.stubGlobal('location', { origin: 'http://127.0.0.1:8766' });
  vi.stubGlobal('Element', LinkTarget);
  vi.stubGlobal('document', {
    documentElement: { style, dataset },
    addEventListener: (name: string, callback: (event: MouseEvent) => void) =>
      handlers.set(name, callback),
    removeEventListener: (name: string, callback: (event: MouseEvent) => void) => {
      if (handlers.get(name) === callback) handlers.delete(name);
    },
  });
  const module = await import('./native');
  return { module, handlers, style, dataset, browserRead, browserWrite, open };
}
function click(
  href: string,
  options: { target?: string; download?: string; button?: number; prevented?: boolean } = {},
) {
  const anchor: Anchor = {
    href,
    target: options.target ?? '',
    download: options.download ?? '',
    hasAttribute: (name) => name === 'download' && options.download !== undefined,
  };
  const event = {
    target: new LinkTarget(anchor),
    button: options.button ?? 0,
    defaultPrevented: options.prevented ?? false,
    preventDefault: vi.fn(() => {
      event.defaultPrevented = true;
    }),
  };
  return event as unknown as MouseEvent & { preventDefault: ReturnType<typeof vi.fn> };
}
async function settle() {
  for (let count = 0; count < 5; count++) await Promise.resolve();
}
beforeEach(() => {
  vi.resetModules();
  vi.resetAllMocks();
  bridge.invoke.mockResolvedValue({ os: 'macos', version: '0.5.0' });
  bridge.listen.mockResolvedValue(bridge.stop);
  bridge.setZoom.mockResolvedValue();
  bridge.attention.mockResolvedValue();
  bridge.openUrl.mockResolvedValue();
  bridge.openPath.mockResolvedValue();
  bridge.reveal.mockResolvedValue();
  bridge.readText.mockResolvedValue('native clipboard');
  bridge.writeText.mockResolvedValue();
  bridge.enable.mockResolvedValue();
  bridge.disable.mockResolvedValue();
  bridge.isEnabled.mockResolvedValue(true);
  bridge.granted.mockResolvedValue(false);
  bridge.request.mockResolvedValue('denied');
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetAllMocks();
});

describe('capability routing', () => {
  it('uses browser clipboard and links without desktop IPC or native listeners', async () => {
    const { module, browserRead, browserWrite, handlers, style, open } = await setup(false);
    expect(module.native).toBe(false);
    expect(await module.readClipboard()).toBe('browser clipboard');
    await module.writeClipboard('Exact browser text.');
    await module.externalLink('https://example.invalid/help');
    await module.applyNativeZoom(1.4);
    module.connectNative(vi.fn(), vi.fn())();
    expect(browserRead).toHaveBeenCalledOnce();
    expect(browserWrite).toHaveBeenCalledExactlyOnceWith('Exact browser text.');
    expect(open).toHaveBeenCalledExactlyOnceWith(
      'https://example.invalid/help',
      '_blank',
      'noopener,noreferrer',
    );
    expect(bridge.invoke).not.toHaveBeenCalled();
    expect(bridge.listen).not.toHaveBeenCalled();
    expect(bridge.readText).not.toHaveBeenCalled();
    expect(bridge.writeText).not.toHaveBeenCalled();
    expect(bridge.openUrl).not.toHaveBeenCalled();
    expect(bridge.setZoom).not.toHaveBeenCalled();
    expect(style.setProperty).not.toHaveBeenCalled();
    expect(handlers.size).toBe(0);
  });
  it('routes native clipboard saves paths autostart engine controls and attention exactly', async () => {
    const { module, browserRead, browserWrite } = await setup();
    expect(await module.readClipboard()).toBe('native clipboard');
    await module.writeClipboard('Exact native text.');
    bridge.invoke.mockResolvedValueOnce('/Desktop/test/export.md').mockResolvedValueOnce(null);
    expect(await module.saveNativeFile('export.md', 'Complete content.')).toBe(
      '/Desktop/test/export.md',
    );
    expect(await module.saveNativeFile('cancel.md', 'Draft.')).toBeNull();
    await module.openNativePath('/Desktop/test/project');
    await module.revealNativePath('/Desktop/test/project/file.ts');
    expect(await module.autostartEnabled()).toBe(true);
    await module.setAutostart(true);
    await module.setAutostart(false);
    await module.restartEngine();
    await module.retryEngine();
    await module.quitApplication();
    await module.nativeAttention();
    expect(bridge.writeText).toHaveBeenCalledExactlyOnceWith('Exact native text.');
    expect(bridge.invoke.mock.calls).toEqual([
      ['save_file', { name: 'export.md', contents: 'Complete content.' }],
      ['save_file', { name: 'cancel.md', contents: 'Draft.' }],
      ['restart_engine'],
      ['retry_engine'],
      ['quit_app'],
    ]);
    expect(bridge.openPath).toHaveBeenCalledExactlyOnceWith('/Desktop/test/project');
    expect(bridge.reveal).toHaveBeenCalledExactlyOnceWith('/Desktop/test/project/file.ts');
    expect(bridge.isEnabled).toHaveBeenCalledOnce();
    expect(bridge.enable).toHaveBeenCalledOnce();
    expect(bridge.disable).toHaveBeenCalledOnce();
    expect(bridge.attention).toHaveBeenCalledExactlyOnceWith('informational');
    expect(browserRead).not.toHaveBeenCalled();
    expect(browserWrite).not.toHaveBeenCalled();
  });
  it.each([true, false])('allows only HTTP external links in native=%s', async (native) => {
    const { module, open } = await setup(native);
    for (const url of [
      'file:///Desktop/test/private',
      'data:text/plain,private',
      'javascript:alert(1)',
      'ftp://example.invalid/file',
      'not a URL',
    ])
      await expect(module.externalLink(url)).rejects.toThrow();
    expect(bridge.openUrl).not.toHaveBeenCalled();
    expect(open).not.toHaveBeenCalled();
    await module.externalLink('http://example.invalid/docs');
    await module.externalLink('https://example.invalid/docs');
    expect(native ? bridge.openUrl : open).toHaveBeenCalledTimes(2);
  });
  it('returns only validated desktop information fields', async () => {
    const { module } = await setup();
    bridge.invoke.mockResolvedValue({ os: 'macos', version: '0.5.0', ignored: 'opaque' });
    expect(await module.appInformation()).toEqual({ os: 'macos', version: '0.5.0' });
    expect(bridge.invoke).toHaveBeenCalledExactlyOnceWith('app_info');
  });
  it.each([
    null,
    false,
    1,
    'desktop',
    [],
    {},
    { os: 1, version: 'v' },
    { os: 'macos', version: null },
    { os: 'macos' },
  ])('rejects malformed desktop information %j', async (value) => {
    const { module } = await setup();
    bridge.invoke.mockResolvedValue(value);
    await expect(module.appInformation()).rejects.toThrow('Desktop information is unavailable.');
  });
});

it('serializes native zoom clamps boundaries and recovers after a rejected change', async () => {
  const { module, style } = await setup();
  const first = deferred<void>();
  bridge.setZoom.mockReturnValueOnce(first.promise);
  const high = module.applyNativeZoom(9);
  const failed = expect(high).rejects.toThrow('fixture zoom failure');
  const low = module.applyNativeZoom(0);
  const invalid = module.applyNativeZoom(Number.NaN);
  await settle();
  expect(bridge.setZoom.mock.calls).toEqual([[1.6]]);
  first.reject(new Error('fixture zoom failure'));
  await failed;
  await low;
  await invalid;
  expect(bridge.setZoom.mock.calls).toEqual([[1.6], [0.7], [1]]);
  expect(style.setProperty.mock.calls).toEqual([
    ['--native-top', `${52 / 1.6}px`],
    ['--native-top', `${52 / 0.7}px`],
    ['--native-top', '52px'],
  ]);
});

describe('native notifications', () => {
  it('uses existing permission and delivers exact title and body without prompting', async () => {
    const { module } = await setup();
    bridge.granted.mockResolvedValue(true);
    expect(await module.requestNativeNotifications()).toBe('granted');
    expect(module.nativeNotificationPermission()).toBe('granted');
    expect(bridge.request).not.toHaveBeenCalled();
    await module.sendNativeNotification('Finished', 'Exact body.');
    expect(bridge.notify).toHaveBeenCalledExactlyOnceWith({
      title: 'Finished',
      body: 'Exact body.',
    });
  });
  it.each(['granted', 'denied'] as const)(
    'records requested %s permission and suppresses delivery while OS permission is absent',
    async (permission) => {
      const { module } = await setup();
      expect(module.nativeNotificationPermission()).toBe('default');
      bridge.request.mockResolvedValue(permission);
      expect(await module.requestNativeNotifications()).toBe(permission);
      expect(module.nativeNotificationPermission()).toBe(permission);
      expect(bridge.request).toHaveBeenCalledOnce();
      await module.sendNativeNotification('Finished', 'Body.');
      expect(bridge.notify).not.toHaveBeenCalled();
    },
  );
});

describe('native connection lifetime and link interception', () => {
  it('registers valid actions and releases the listener after registration', async () => {
    const { module, handlers, dataset } = await setup();
    const action = vi.fn(),
      fail = vi.fn();
    bridge.granted.mockResolvedValue(true);
    const dispose = module.connectNative(action, fail);
    await settle();
    expect(bridge.listen).toHaveBeenCalledWith('native-action', expect.any(Function));
    const callback = bridge.listen.mock.calls[0]![1];
    callback({ payload: 'new-session' });
    callback({ payload: { action: 'quit' } });
    expect(action).toHaveBeenCalledExactlyOnceWith('new-session');
    expect(dataset.nativeOs).toBe('macos');
    expect(module.nativeNotificationPermission()).toBe('granted');
    dispose();
    callback({ payload: 'quit' });
    expect(bridge.stop).toHaveBeenCalledOnce();
    expect(handlers.has('click')).toBe(false);
    expect(action).toHaveBeenCalledOnce();
    expect(fail).not.toHaveBeenCalled();
  });
  it('disposal before registration rejects late action information and permission callbacks', async () => {
    const registration = deferred<() => void>(),
      information = deferred<unknown>(),
      permission = deferred<boolean>();
    bridge.listen.mockReturnValue(registration.promise);
    bridge.invoke.mockReturnValue(information.promise);
    bridge.granted.mockReturnValue(permission.promise);
    const { module, handlers, dataset } = await setup();
    const action = vi.fn(),
      fail = vi.fn();
    const dispose = module.connectNative(action, fail);
    const callback = bridge.listen.mock.calls[0]![1];
    dispose();
    callback({ payload: 'quit' });
    registration.resolve(bridge.stop);
    information.resolve({ os: 'macos', version: '0.5.0' });
    permission.resolve(true);
    await settle();
    expect(bridge.stop).toHaveBeenCalledOnce();
    expect(action).not.toHaveBeenCalled();
    expect(dataset.nativeOs).toBeUndefined();
    expect(module.nativeNotificationPermission()).toBe('default');
    expect(handlers.has('click')).toBe(false);
    expect(fail).not.toHaveBeenCalled();
  });
  it('suppresses failed asynchronous registrations after disposal', async () => {
    const registration = deferred<() => void>(),
      information = deferred<unknown>(),
      permission = deferred<boolean>();
    bridge.listen.mockReturnValue(registration.promise);
    bridge.invoke.mockReturnValue(information.promise);
    bridge.granted.mockReturnValue(permission.promise);
    const { module } = await setup();
    const fail = vi.fn();
    module.connectNative(vi.fn(), fail)();
    registration.reject(new Error('listener unavailable'));
    information.reject(new Error('information unavailable'));
    permission.reject(new Error('permission unavailable'));
    await settle();
    expect(fail).not.toHaveBeenCalled();
  });
  it('reports failed registration while the connection remains active', async () => {
    const { module } = await setup();
    const error = new Error('listener unavailable'),
      fail = vi.fn();
    bridge.listen.mockRejectedValueOnce(error);
    const dispose = module.connectNative(vi.fn(), fail);
    await settle();
    expect(fail).toHaveBeenCalledExactlyOnceWith(error);
    dispose();
  });
  it('intercepts only external HTTP links and leaves own origin downloads and unsafe schemes untouched', async () => {
    const { module, handlers } = await setup();
    const fail = vi.fn(),
      dispose = module.connectNative(vi.fn(), fail);
    const listener = handlers.get('click')!;
    for (const event of [
      click('http://127.0.0.1:8766/api/state'),
      click('http://127.0.0.1:8766/export.md', { download: 'export.md' }),
      click('http://127.0.0.1:8766/export.md', { download: '' }),
      click('https://example.invalid/file', { download: 'file.md' }),
      click('https://example.invalid/file', { download: '' }),
      click('file:///Desktop/test/private'),
      click('data:text/plain,private'),
      click('javascript:alert(1)'),
      click('https://example.invalid/help', { button: 1 }),
      click('https://example.invalid/help', { prevented: true }),
    ]) {
      listener(event);
      expect(event.preventDefault).not.toHaveBeenCalled();
    }
    expect(bridge.openUrl).not.toHaveBeenCalled();
    const external = click('https://example.invalid/help');
    listener(external);
    await settle();
    expect(external.preventDefault).toHaveBeenCalledOnce();
    expect(bridge.openUrl).toHaveBeenCalledExactlyOnceWith('https://example.invalid/help');
    dispose();
    listener(click('https://example.invalid/late'));
    expect(bridge.openUrl).toHaveBeenCalledOnce();
    expect(fail).not.toHaveBeenCalled();
  });
  it('reports malformed links without throwing or opening another application', async () => {
    const { module, handlers } = await setup();
    const fail = vi.fn(),
      dispose = module.connectNative(vi.fn(), fail);
    await settle();
    const event = click('http://[');
    expect(() => handlers.get('click')!(event)).not.toThrow();
    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(fail).toHaveBeenCalledOnce();
    expect(bridge.openUrl).not.toHaveBeenCalled();
    dispose();
  });
  it('allows an explicit own origin new tab and suppresses an opener failure after disposal', async () => {
    const opening = deferred<void>();
    bridge.openUrl.mockReturnValue(opening.promise);
    const { module, handlers } = await setup();
    const fail = vi.fn(),
      dispose = module.connectNative(vi.fn(), fail);
    const event = click('http://127.0.0.1:8766/help', { target: '_blank' });
    handlers.get('click')!(event);
    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(bridge.openUrl).toHaveBeenCalledExactlyOnceWith('http://127.0.0.1:8766/help');
    dispose();
    opening.reject(new Error('opener unavailable'));
    await settle();
    expect(fail).not.toHaveBeenCalled();
  });
});
