/** Owns how background jobs, loops and schedules read, and the job log view's state. */
import { writable } from 'svelte/store';
import type { Job } from '../gen/Job';
import type { JobLogResponse } from '../gen/JobLogResponse';
import type { Session } from '../gen/Session';

/** A stored job record, or null when the value is not one. */
export function asJob(value: unknown): Job | null {
  if (!value || typeof value !== 'object') return null;
  const job = value as Partial<Job>;
  return typeof job.id === 'string' &&
    typeof job.cmd === 'string' &&
    typeof job.started === 'number'
    ? (job as Job)
    : null;
}

export function runningJobs(session: Session | null): Job[] {
  return (session?.jobs ?? []).map(asJob).filter((job): job is Job => !!job && job.ended === null);
}

/** "42s" style elapsed time: minutes and seconds under an hour, then hours and minutes. */
export function elapsed(since: number, now = Date.now() / 1000): string {
  const s = Math.max(0, Math.floor(now - since));
  return s < 3600
    ? `${Math.floor(s / 60)}m${String(s % 60).padStart(2, '0')}s`
    : `${Math.floor(s / 3600)}h${String(Math.floor(s / 60) % 60).padStart(2, '0')}m`;
}

export function formatEvery(seconds: number): string {
  if (seconds % 3600 === 0) return `${seconds / 3600}h`;
  if (seconds % 60 === 0) return `${seconds / 60}m`;
  return `${seconds}s`;
}

/** The running-job line: elapsed time, check-in interval and who started it. */
export function jobDetail(job: Job, now = Date.now() / 1000): string {
  return (
    elapsed(job.started, now) +
    (job.every ? ` · check-in every ${Math.round(job.every / 60)}m` : '') +
    (job.by === 'agent' ? ' · started by agy' : '')
  );
}

/** How a job event reads: started, stopped, exit code, or finished with an unknown code. */
export function jobStatus(phase: string, job: Job): string {
  if (phase !== 'ended') return 'started';
  if (job.stopped) return 'stopped';
  if (job.code === null) return 'finished';
  return `exit ${job.code}`;
}

export function jobFailed(phase: string, job: Job): boolean {
  return phase === 'ended' && !job.stopped && job.code !== null && job.code !== 0;
}

export interface LoopConfig {
  every: number;
  prompt: string;
  next: number;
  at?: string;
}

export function asLoop(value: unknown): LoopConfig | null {
  if (!value || typeof value !== 'object') return null;
  const loop = value as Partial<LoopConfig>;
  return typeof loop.prompt === 'string' && typeof loop.next === 'number'
    ? (loop as LoopConfig)
    : null;
}

/** "every 15m · next Wed 14:05" or "daily at 03:00 · next Thu 03:00". */
export function loopDetail(loop: LoopConfig): string {
  const next = new Date(loop.next * 1000).toLocaleString([], {
    weekday: 'short',
    hour: '2-digit',
    minute: '2-digit',
  });
  return `${loop.at ? `daily at ${loop.at}` : `every ${formatEvery(loop.every)}`} · next ${next}`;
}

/** The job whose log is open, with its text. */
export const jobLog = writable<JobLogResponse | null>(null);
