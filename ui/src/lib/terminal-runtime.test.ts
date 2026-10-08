/** Proves xterm ownership, replay reset, error color and the exact resize debounce. */
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { createApi } from './api';
import { shellFor, fitShell, restartShell } from './terminal';
const fixture = vi.hoisted(() => ({ terminals: [] as FakeTerminal[], fits: [] as FakeFit[] }));
class FakeTerminal {
  cols = 80;
  rows = 24;
  write = vi.fn();
  reset = vi.fn();
  open = vi.fn();
  loadAddon = vi.fn();
  input: (text: string) => void = () => {};
  constructor(public options: Record<string, unknown>) {
    fixture.terminals.push(this);
  }
  onData(callback: (text: string) => void) {
    this.input = callback;
  }
}
class FakeFit {
  fit = vi.fn();
  constructor() {
    fixture.fits.push(this);
  }
}
vi.mock('@xterm/xterm', () => ({ Terminal: FakeTerminal }));
vi.mock('@xterm/addon-fit', () => ({ FitAddon: FakeFit }));
class FakeSource {
  static instances: FakeSource[] = [];
  onmessage: ((message: { data: string }) => void) | null = null;
  close = vi.fn();
  constructor(public url: string) {
    FakeSource.instances.push(this);
  }
  frame(value: unknown) {
    this.onmessage?.({ data: JSON.stringify(value) });
  }
}
beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal('document', { documentElement: {}, createElement: () => ({ className: '' }) });
  vi.stubGlobal('getComputedStyle', () => ({ getPropertyValue: () => '' }));
  vi.stubGlobal('EventSource', FakeSource);
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
function api() {
  return {
    ...createApi('test'),
    termStart: vi.fn(async () => ({ alive: true, shell: 'fixture-shell', cwd: '/test/project' })),
    termResize: vi.fn(async () => ({ ok: true })),
    termInput: vi.fn(async () => ({ ok: true })),
  };
}
it('uses 5000-line scrollback and resets exactly once for each first output replay including restart', async () => {
  const client = api(),
    shell = await shellFor('replay-fixture', client, (id) => `/stream/${id}`);
  const term = fixture.terminals.at(-1)!;
  expect(term.options).toMatchObject({ scrollback: 5000 });
  expect(await shellFor('replay-fixture', client, () => '/unused')).toBe(shell);
  expect(client.termStart).toHaveBeenCalledOnce();
  const source = FakeSource.instances.at(-1)!;
  source.frame({ d: btoa('screen replay') });
  source.frame({ d: btoa('new output') });
  expect(term.reset).toHaveBeenCalledOnce();
  expect([...(term.write.mock.calls[0]![0] as Uint8Array)]).toEqual([
    ...new TextEncoder().encode('screen replay'),
  ]);
  await restartShell(shell, client, () => '/restart');
  expect(source.close).toHaveBeenCalledOnce();
  FakeSource.instances.at(-1)!.frame({ d: btoa('restart replay') });
  expect(term.reset).toHaveBeenCalledTimes(2);
  FakeSource.instances.at(-1)!.frame({ exit: true });
  expect(shell.alive).toBe(false);
  expect(shell.title).toContain('exited');
});
it('fits immediately and posts only the last dimensions after 80ms', async () => {
  const client = api(),
    shell = await shellFor('resize-fixture', client, () => '/stream');
  const term = fixture.terminals.at(-1)!;
  term.cols = 100;
  term.rows = 30;
  fitShell(shell, client);
  vi.advanceTimersByTime(79);
  expect(client.termResize).not.toHaveBeenCalled();
  term.cols = 120;
  term.rows = 35;
  fitShell(shell, client);
  vi.advanceTimersByTime(79);
  expect(client.termResize).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  expect(client.termResize).toHaveBeenCalledExactlyOnceWith('resize-fixture', 120, 35);
  expect(fixture.fits.at(-1)?.fit).toHaveBeenCalledTimes(3);
});
it('renders both startup and input failures in ANSI red without throwing', async () => {
  const client = api();
  client.termStart.mockRejectedValueOnce(new Error('startup refused'));
  await shellFor('error-fixture', client, () => '/stream');
  const term = fixture.terminals.at(-1)!;
  expect(term.write).toHaveBeenCalledWith('\r\n\x1b[31mstartup refused\x1b[0m\r\n');
  client.termInput.mockRejectedValueOnce(new Error('input refused'));
  term.input('typed');
  await vi.advanceTimersByTimeAsync(0);
  expect(term.write).toHaveBeenCalledWith('\r\n\x1b[31minput refused\x1b[0m\r\n');
});
