/** Proves job, loop and schedule wording. */
import { describe, expect, it } from 'vitest';
import type { Job } from '../gen/Job';
import type { Session } from '../gen/Session';
import {
  asJob,
  asLoop,
  elapsed,
  formatEvery,
  jobDetail,
  jobFailed,
  jobStatus,
  loopDetail,
  runningJobs,
} from './jobs';

const job: Job = {
  id: 'a1b2c3d4',
  cmd: 'make train',
  pid: 7,
  log: '/l',
  started: 1000,
  ended: null,
  code: null,
  every: 600,
  notify: true,
  by: 'agent',
  stopped: false,
};

describe('jobs', () => {
  it('read running jobs and their details', () => {
    const session = {
      jobs: [job, { ...job, id: 'x', ended: 2000 }, { junk: true }],
    } as unknown as Session;
    expect(runningJobs(session).map((j) => j.id)).toEqual(['a1b2c3d4']);
    expect(asJob({ id: 1 })).toBeNull();
    expect(elapsed(1000, 1125)).toBe('2m05s');
    expect(elapsed(1000, 1000 + 7260)).toBe('2h01m');
    expect(jobDetail(job, 1065)).toBe('1m05s · check-in every 10m · started by agy');
    expect(jobDetail({ ...job, every: 0, by: 'user' }, 1001)).toBe('0m01s');
  });
  it('distinguish started, stopped, success, failure and unknown exits', () => {
    expect(jobStatus('started', job)).toBe('started');
    expect(jobStatus('ended', { ...job, stopped: true, code: -1 })).toBe('stopped');
    expect(jobStatus('ended', { ...job, code: 0 })).toBe('exit 0');
    expect(jobStatus('ended', { ...job, code: 2 })).toBe('exit 2');
    expect(jobStatus('ended', job)).toBe('finished');
    expect(jobFailed('ended', { ...job, code: 2 })).toBe(true);
    expect(jobFailed('ended', { ...job, code: 0 })).toBe(false);
    expect(jobFailed('ended', job)).toBe(false);
  });
  it('describe loops and daily schedules', () => {
    expect(formatEvery(900)).toBe('15m');
    expect(formatEvery(7200)).toBe('2h');
    expect(formatEvery(90)).toBe('90s');
    expect(asLoop(null)).toBeNull();
    const loop = asLoop({ every: 900, prompt: 'Ping', next: 0 })!;
    expect(loopDetail(loop)).toMatch(/^every 15m · next /);
    expect(loopDetail({ ...loop, every: 86400, at: '03:00' })).toMatch(/^daily at 03:00 · next /);
  });
});
