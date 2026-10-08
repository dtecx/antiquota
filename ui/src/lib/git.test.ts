/** Proves git review helpers, edit history grouping and folder navigation helpers. */
import { get } from 'svelte/store';
import { afterEach, describe, expect, it } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import {
  asStatus,
  breadcrumbs,
  canMerge,
  codeLabel,
  editHistory,
  humanSize,
  parentPath,
  statusLine,
  viewer,
  viewFile,
  type FileText,
} from './git';

const worktree = {
  repo: '/r',
  path: '/w',
  branch: 'antiquota/x-ab12',
  base: 'main',
  base_sha: 'abc',
};

describe('git review', () => {
  it('reads statuses and when merge is possible', () => {
    expect(asStatus({})).toBeNull();
    const main = asStatus({ root: '/r', branch: 'main', files: [], worktree: null })!;
    expect(statusLine(main)).toBe('main · /r');
    expect(canMerge(main)).toBe(false);
    const ahead = { ...main, branch: worktree.branch, worktree, ahead: 2, merged: false };
    expect(statusLine(ahead)).toBe('antiquota/x-ab12 from main · 2 ahead');
    expect(canMerge(ahead)).toBe(true);
    expect(canMerge({ ...ahead, merged: true })).toBe(false);
    expect(statusLine({ ...ahead, merged: true })).toBe(
      'antiquota/x-ab12 from main · 2 ahead · merged',
    );
    expect(canMerge({ ...ahead, ahead: 0 })).toBe(false);
    expect(codeLabel('??')).toBe('new');
    expect(codeLabel('M')).toBe('modified');
  });
  it('groups the agent edits by file, newest first', () => {
    const tool = (ts: number, file: string, diff?: string): EventRecord => ({
      t: 'tool',
      ts,
      turn: 1,
      step: ts,
      name: 'write_to_file',
      params: { TargetFile: file },
      state: 'done',
      ...(diff ? { diff } : {}),
    });
    const history = editHistory([
      tool(1, 'a.ts', 'd1'),
      tool(2, 'b.ts', 'd2'),
      tool(3, 'a.ts', 'd3'),
      tool(4, 'c.ts'),
    ]);
    expect(history.map((h) => [h.file, h.diffs])).toEqual([
      ['a.ts', ['d1', 'd3']],
      ['b.ts', ['d2']],
    ]);
  });
  it('navigates folders and prints sizes', () => {
    expect(breadcrumbs('')).toEqual([{ label: 'project', path: '' }]);
    expect(breadcrumbs('src/lib')).toEqual([
      { label: 'project', path: '' },
      { label: 'src', path: 'src' },
      { label: 'lib', path: 'src/lib' },
    ]);
    expect(parentPath('src/lib')).toBe('src');
    expect(parentPath('src')).toBe('');
    expect(humanSize(12)).toBe('12 B');
    expect(humanSize(2048)).toBe('2.0 KB');
    expect(humanSize(3 * 1024 * 1024)).toBe('3.0 MB');
  });
});

function deferredFile() {
  let resolve: (file: FileText | null) => void = () => {};
  let reject: (error: Error) => void = () => {};
  const promise = new Promise<FileText | null>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}
function file(path: string): FileText {
  return { path, rel: path, text: `Contents of ${path}` };
}
afterEach(() => viewer.set(null));

describe('shared file viewer ownership', () => {
  it.each(['older first', 'newer first'])(
    'keeps the newer read authoritative when requests resolve %s',
    async (order) => {
      const previous = file('previous.ts'),
        older = deferredFile(),
        newer = deferredFile(),
        latest = file('newer.ts');
      viewer.set(previous);
      const oldRead = viewFile(
        () => older.promise,
        () => true,
      );
      const newRead = viewFile(
        () => newer.promise,
        () => true,
      );
      if (order === 'older first') {
        older.resolve(file('older.ts'));
        await oldRead;
        expect(get(viewer)).toBe(previous);
        newer.resolve(latest);
        await newRead;
      } else {
        newer.resolve(latest);
        await newRead;
        older.resolve(file('older.ts'));
        await oldRead;
      }
      expect(get(viewer)).toBe(latest);
    },
  );
  it('does not publish after the initiating selection or component becomes stale', async () => {
    const response = deferredFile(),
      previous = file('previous.ts');
    let current = true;
    viewer.set(previous);
    const reading = viewFile(
      () => response.promise,
      () => current,
    );
    current = false;
    response.resolve(file('stale.ts'));
    await reading;
    expect(get(viewer)).toBe(previous);
  });
  it.each(['new direct viewer', 'dismissed viewer'])(
    'keeps a %s instead of replacing it with a pending read',
    async (change) => {
      const response = deferredFile(),
        newer = change === 'new direct viewer' ? file('direct.ts') : null;
      viewer.set(file('previous.ts'));
      const reading = viewFile(
        () => response.promise,
        () => true,
      );
      viewer.set(newer);
      response.resolve(file('pending.ts'));
      await reading;
      expect(get(viewer)).toBe(newer);
    },
  );
  it('propagates a rejected read and prevents an older request from publishing afterward', async () => {
    const older = deferredFile(),
      newest = deferredFile(),
      previous = file('previous.ts'),
      error = new Error('Read failed');
    viewer.set(previous);
    const oldRead = viewFile(
      () => older.promise,
      () => true,
    );
    const newRead = viewFile(
      () => newest.promise,
      () => true,
    );
    const rejected = expect(newRead).rejects.toBe(error);
    newest.reject(error);
    await rejected;
    older.resolve(file('older.ts'));
    await oldRead;
    expect(get(viewer)).toBe(previous);
  });
  it('does not publish a null result or revive an older pending request', async () => {
    const older = deferredFile(),
      previous = file('previous.ts');
    viewer.set(previous);
    const oldRead = viewFile(
      () => older.promise,
      () => true,
    );
    await viewFile(
      async () => null,
      () => true,
    );
    older.resolve(file('older.ts'));
    await oldRead;
    expect(get(viewer)).toBe(previous);
  });
});
