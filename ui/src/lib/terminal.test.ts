/** Proves terminal frame decoding and ordered keystroke delivery. */
import { describe, expect, it, vi } from 'vitest';
import { decodeFrame, inputQueue } from './terminal';

describe('terminal frames', () => {
  it('decode base64 into raw bytes, including non-UTF-8 ones', () => {
    expect(new TextDecoder().decode(decodeFrame('aGVsbG8NCg=='))).toBe('hello\r\n');
    expect([...decodeFrame('//4A')]).toEqual([255, 254, 0]);
    expect(decodeFrame('')).toHaveLength(0);
  });
});

describe('terminal input', () => {
  it('sends one request at a time, in order, joining keys typed meanwhile', async () => {
    const sent: string[] = [];
    const finish: (() => void)[] = [];
    const push = inputQueue(
      (data) =>
        new Promise<void>((resolve) => {
          sent.push(data);
          finish.push(resolve);
        }),
      () => {},
    );
    for (const key of 'printf') push(key);
    expect(sent).toEqual(['p']);
    finish.shift()?.();
    await vi.waitFor(() => expect(sent).toEqual(['p', 'rintf']));
    finish.shift()?.();
    push('\r');
    await vi.waitFor(() => expect(sent).toEqual(['p', 'rintf', '\r']));
  });
  it('reports a failed request and keeps sending later keys', async () => {
    const failures: unknown[] = [];
    const send = vi
      .fn<(data: string) => Promise<void>>()
      .mockRejectedValueOnce(new Error('the shell has exited: restart it'))
      .mockResolvedValue();
    const push = inputQueue(send, (error) => failures.push(error));
    push('a');
    push('b');
    await vi.waitFor(() => expect(send.mock.calls).toEqual([['a'], ['b']]));
    expect(String(failures[0])).toContain('the shell has exited');
  });
});
