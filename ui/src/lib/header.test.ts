/** Proves session header paths, state labels and cumulative token counts. */
import { describe, expect, it } from 'vitest';
import { clockTime } from './account-format';
import { fixtureSession } from './fixtures.test-support';
import { headerPath, headerStatus, headerUsage, shortHeaderPath } from './header';

describe('session header labels', () => {
  it.each([
    ['/Users/tester/project', '~/project'],
    ['/Users/tester', '~'],
    ['/home/tester/project', '~/project'],
    ['/home/tester', '~'],
    ['/Desktop/test/project', '/Desktop/test/project'],
    ['/Users/', '/Users/'],
    ['relative/project', 'relative/project'],
    ['', ''],
  ])('shortens %s without hiding its complete tooltip', (cwd, label) => {
    expect(shortHeaderPath(cwd)).toBe(label);
    expect(headerPath(cwd, null)).toEqual({ label, tooltip: cwd });
  });
  it('shows a worktree branch while retaining its full path, base and repository', () => {
    const worktree = {
      path: '/Desktop/test/worktrees/feature',
      repo: '/Desktop/test/project',
      branch: 'feature',
      base: 'rust',
      base_sha: 'synthetic',
    };
    expect(headerPath(worktree.path, worktree)).toEqual({
      label: 'feature',
      tooltip: '/Desktop/test/worktrees/feature · from rust in /Desktop/test/project',
    });
  });
  it.each([
    ['idle', ''],
    ['running', 'working'],
    ['approval', 'needs your approval'],
    ['waiting', 'waiting for an account'],
    ['checking', 'running check'],
    ['verifying', 'verifier reviewing'],
    ['sleeping', 'sleeping'],
    ['unknown', 'unknown'],
  ])('labels %s', (status, label) => {
    expect(headerStatus({ ...fixtureSession(), status })).toBe(label);
  });
  it('includes the sleeping deadline in local clock format and hides it for other states', () => {
    expect(headerStatus({ status: 'sleeping', status_until: 1234 })).toBe(
      `sleeping until ${clockTime(1234)}`,
    );
    expect(headerStatus({ status: 'idle', status_until: 1234 })).toBe('');
    expect(headerStatus({ status: 'running', status_until: 1234 })).toBe('working');
  });
  it.each([
    [{ input: 0, output: 0, thinking: 100 }, ''],
    [{ input: 100, output: 0, thinking: 0 }, '100 in · 0 out'],
    [{ input: 0, output: 100, thinking: 0 }, '0 in · 100 out'],
    [{ input: 1234, output: 2_500_000, thinking: 100 }, '1.2k in · 2.5M out'],
  ])('formats cumulative input and output for %j', (usage, label) => {
    expect(headerUsage(usage)).toBe(label);
  });
});
