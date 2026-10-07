/** Proves display boundaries without using locale-dependent date assertions. */
import { describe, expect, it } from 'vitest';
import { duration, formatTokens, projectName, relativeTime } from './format';
describe('workbench formatting', () => {
  it('keeps counters legible across thousands and millions', () => {
    expect([0, 999, 1_000, 24_600, 1_000_000].map(formatTokens)).toEqual([
      '0',
      '999',
      '1.0k',
      '24.6k',
      '1.0M',
    ]);
  });
  it('handles future timestamps and minute/hour boundaries', () => {
    expect(relativeTime(110, 100)).toBe('now');
    expect(relativeTime(40, 100)).toBe('1m');
    expect(relativeTime(100, 3_700)).toBe('1h');
  });
  it('names folders on all three supported platforms', () => {
    expect(projectName('/Users/name/project/')).toBe('project');
    expect(projectName('C:\\Users\\name\\project\\')).toBe('project');
    expect(projectName('/')).toBe('/');
  });
  it('formats short and long turn durations', () => {
    expect([0.2, 2.1, 35, 63].map(duration)).toEqual(['<1s', '2.1s', '35s', '1m 3s']);
  });
});
