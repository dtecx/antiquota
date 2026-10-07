/** Proves git review helpers, edit history grouping and folder navigation helpers. */
import { describe, expect, it } from 'vitest';
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
