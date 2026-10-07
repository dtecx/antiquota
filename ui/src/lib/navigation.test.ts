/** Proves project navigation hides archives and preserves source session ordering. */
import { expect, it } from 'vitest';
import { fixtureSession } from './fixtures.test-support';
import { groupSessions } from './navigation';
it('groups folders, pins before recent sessions, and combines filter terms', () => {
  const sessions = [
    { ...fixtureSession('old'), updated: 1, pinned: true },
    { ...fixtureSession('new'), updated: 100 },
    { ...fixtureSession('archive'), archived: true },
    { ...fixtureSession('other'), cwd: '/Desktop/test/other' },
  ];
  const groups = groupSessions(sessions, 'fixture PROJECT');
  expect(groups).toHaveLength(1);
  expect(groups[0]?.sessions.map((session) => session.id)).toEqual(['old', 'new']);
  expect(sessions.map((session) => session.id)).toEqual(['old', 'new', 'archive', 'other']);
  expect(groupSessions(sessions, 'missing')).toEqual([]);
});
