/** Proves terminal frame decoding. */
import { describe, expect, it } from 'vitest';
import { decodeFrame } from './terminal';

describe('terminal frames', () => {
  it('decode base64 into raw bytes, including non-UTF-8 ones', () => {
    expect(new TextDecoder().decode(decodeFrame('aGVsbG8NCg=='))).toBe('hello\r\n');
    expect([...decodeFrame('//4A')]).toEqual([255, 254, 0]);
    expect(decodeFrame('')).toHaveLength(0);
  });
});
