/** Proves tray bookkeeping, pasted names, caret insertion and the WAV encoding. */
import { get } from 'svelte/store';
import { describe, expect, it, vi } from 'vitest';
import {
  audioExtension,
  clearTrays,
  mediaNotice,
  removeSent,
  trays,
  uploadFiles,
  extensionLabel,
  insertAt,
  pastedName,
  peakOf,
  pendingFile,
  readyPaths,
  recordingLabel,
  recordingProblem,
} from './media';

describe('attachments', () => {
  it('sends only finished uploads, in order', () => {
    const a = { ...pendingFile(new File(['a'], 'a.txt'), 'a.txt'), uploading: false, path: '/a' };
    const b = pendingFile(new File(['b'], 'b.txt'), 'b.txt');
    const c = { ...pendingFile(new File(['c'], 'c.txt'), 'c.txt'), uploading: false, path: '/c' };
    expect(readyPaths([a, b, c])).toEqual(['/a', '/c']);
    expect(a.key).toBeLessThan(b.key);
    expect(a.image).toBe(false);
    expect(pendingFile(new File(['x'], 'x.png', { type: 'image/png' }), 'x.png').image).toBe(true);
  });
  it('names pasted screenshots by time and labels file kinds', () => {
    const at = new Date(2026, 9, 8, 9, 5, 7);
    expect(pastedName('image.png', at)).toBe('screenshot-090507.png');
    expect(pastedName('', at)).toBe('screenshot-090507.png');
    expect(pastedName('notes.md', at)).toBe('notes.md');
    expect(extensionLabel('report.final.json')).toBe('json');
    expect(extensionLabel('Makefile')).toBe('file');
    expect(extensionLabel('.env')).toBe('file');
  });
});

describe('uploads', () => {
  it('fills the tray, refuses oversized files and drops failed uploads', async () => {
    trays.set({});
    const attach = vi
      .fn()
      .mockResolvedValueOnce({ name: 'a.txt', path: '/s/a.txt', image: false, size: 1 })
      .mockRejectedValueOnce(new Error('attachments are limited to 25 MB'));
    const failures: unknown[] = [];
    const huge = new File(['x'], 'huge.bin');
    Object.defineProperty(huge, 'size', { value: 25_000_001 });
    await uploadFiles(
      { attach },
      's',
      [new File(['a'], 'a.txt'), huge, new File(['b'], 'b.txt')],
      (error) => failures.push(error),
    );
    expect(get(mediaNotice)).toBe('huge.bin is over 25 MB');
    expect(attach).toHaveBeenCalledTimes(2);
    expect(get(trays).s?.map((f) => [f.name, f.uploading, f.path])).toEqual([
      ['a.txt', false, '/s/a.txt'],
    ]);
    expect(String(failures[0])).toContain('limited to 25 MB');
    removeSent('s', ['/s/a.txt']);
    expect(get(trays).s).toEqual([]);
  });
  it('clears other sessions when one opens', () => {
    const file = { ...pendingFile(new File(['a'], 'a.txt'), 'a.txt'), uploading: false };
    trays.set({ a: [file], b: [file] });
    clearTrays('b');
    expect(Object.keys(get(trays))).toEqual(['b']);
    clearTrays('c');
    expect(get(trays)).toEqual({});
  });
});

describe('dictation helpers', () => {
  it('inserts words at the selection with one separating space', () => {
    expect(insertAt('', 0, 0, 'Hello')).toEqual({ value: 'Hello', caret: 5 });
    expect(insertAt('Fix it', 6, 6, 'now')).toEqual({ value: 'Fix it now', caret: 10 });
    expect(insertAt('Fix it ', 7, 7, 'now')).toEqual({ value: 'Fix it now', caret: 10 });
    expect(insertAt('Fix the bug', 4, 7, 'that')).toEqual({ value: 'Fix that bug', caret: 8 });
  });
  it('rejects short and silent recordings', () => {
    expect(recordingProblem(0.4, 0.5)).toBe('short');
    expect(recordingProblem(2, 0.009)).toBe('silent');
    expect(recordingProblem(2, 0.2)).toBeNull();
    expect(peakOf(new Float32Array([0.1, -0.7, 0.3]))).toBeCloseTo(0.7);
    expect(recordingLabel(65.9)).toBe('● 1:05 · Esc cancels');
  });
  it('names recordings by the container the recorder produced', () => {
    expect(audioExtension('audio/ogg;codecs=opus')).toBe('ogg');
    expect(audioExtension('audio/mp4')).toBe('m4a');
    expect(audioExtension('audio/webm;codecs=opus')).toBe('webm');
    expect(audioExtension('audio/wav')).toBe('wav');
    expect(audioExtension('')).toBe('webm');
  });
});
it('keeps both visible pane attachment trays while clearing all other sessions', () => {
  const file = { ...pendingFile(new File(['a'], 'a.txt'), 'a.txt'), uploading: false };
  trays.set({ a: [file], b: [file], c: [file] });
  clearTrays(['a', 'b']);
  expect(Object.keys(get(trays))).toEqual(['a', 'b']);
  clearTrays(['b']);
  expect(Object.keys(get(trays))).toEqual(['b']);
  clearTrays([]);
});
