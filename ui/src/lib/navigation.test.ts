/** Proves visible project grouping, independent split choices, and wrapped navigation. */
import { expect, it } from 'vitest';
import { fixtureSession } from './fixtures.test-support';
import { groupSessions, PINNED, splitChoices, stepSession } from './navigation';
it('separates pins, orders projects by recency, combines filter terms, and preserves input', () => {
  const sessions = [
    { ...fixtureSession('old'), updated: 1, pinned: true },
    { ...fixtureSession('new'), updated: 100, cwd: '/Desktop/test/project-new' },
    { ...fixtureSession('older'), updated: 50, cwd: '/Desktop/test/project-new' },
    { ...fixtureSession('pin-new'), updated: 200, pinned: true },
    { ...fixtureSession('archive'), archived: true },
    { ...fixtureSession('other'), updated: 300, cwd: '/Desktop/test/project-other' },
  ];
  const original = structuredClone(sessions);
  const groups = groupSessions(sessions, 'fixture PROJECT');
  expect(groups.map((group) => [group.cwd, group.sessions.map((session) => session.id)])).toEqual([
    [PINNED, ['pin-new', 'old']],
    ['/Desktop/test/project-other', ['other']],
    ['/Desktop/test/project-new', ['new', 'older']],
  ]);
  expect(sessions).toEqual(original);
  expect(groupSessions(sessions, 'missing')).toEqual([]);
});

it('shows archives only when requested or currently selected, while retaining filtering', () => {
  const sessions = [
    fixtureSession('visible'),
    { ...fixtureSession('current'), archived: true },
    { ...fixtureSession('hidden'), archived: true },
  ];
  const ids = (options = {}) =>
    groupSessions(sessions, '', options).flatMap((group) =>
      group.sessions.map((session) => session.id),
    );
  expect(ids()).toEqual(['visible']);
  expect(ids({ current: 'current' })).toEqual(['visible', 'current']);
  expect(ids({ showArchived: true })).toEqual(['visible', 'current', 'hidden']);
  expect(groupSessions(sessions, 'missing', { current: 'current' })).toEqual([]);
});

it('puts a worktree session under its repository', () => {
  const groups = groupSessions(
    [
      { ...fixtureSession('a'), cwd: '/repo', updated: 2 },
      {
        ...fixtureSession('b'),
        cwd: '/data/worktrees/repo-b',
        updated: 1,
        worktree: {
          repo: '/repo',
          path: '/data/worktrees/repo-b',
          branch: 'x',
          base: 'main',
          base_sha: 's',
        },
      },
    ],
    '',
  );
  expect(groups.map((g) => [g.cwd, g.sessions.map((s) => s.id)])).toEqual([['/repo', ['a', 'b']]]);
});

it('limits split choices to fourteen recent distinct unarchived other sessions and retains status', () => {
  const sessions = [
    { ...fixtureSession('current'), updated: 100 },
    { ...fixtureSession('archive'), updated: 99, archived: true },
    { ...fixtureSession('choice-19'), updated: 0 },
    ...Array.from({ length: 20 }, (_, index) => ({
      ...fixtureSession(`choice-${index}`),
      updated: index,
      status: index === 19 ? 'running' : 'idle',
    })),
  ];
  const original = structuredClone(sessions);
  const choices = splitChoices(sessions, 'current');
  expect(choices.map((session) => session.id)).toEqual(
    Array.from({ length: 14 }, (_, index) => `choice-${19 - index}`),
  );
  expect(choices[0]?.status).toBe('running');
  expect(sessions).toEqual(original);
  expect(splitChoices([fixtureSession('only')], 'only')).toEqual([]);
});

it('steps through visible pinned and project order and wraps at either end', () => {
  const groups = groupSessions(
    [
      { ...fixtureSession('pin'), pinned: true, updated: 1 },
      { ...fixtureSession('new'), updated: 100 },
      { ...fixtureSession('old'), updated: 50, cwd: '/Desktop/test/other' },
      { ...fixtureSession('hidden'), archived: true },
    ],
    '',
  );
  expect(stepSession(groups, 'pin', 1)).toBe('new');
  expect(stepSession(groups, 'new', -1)).toBe('pin');
  expect(stepSession(groups, 'old', 1)).toBe('pin');
  expect(stepSession(groups, 'pin', -1)).toBe('old');
  expect(stepSession(groups, 'pin', -4)).toBe('old');
  expect(stepSession(groups, 'missing', 1)).toBe('pin');
  expect(stepSession(groups, 'missing', -1)).toBe('old');
  expect(stepSession([], 'missing', 1)).toBeUndefined();
});
