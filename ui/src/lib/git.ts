/** Owns git review data, the agent's edit history, project files and the side drawer state. */
import { writable } from 'svelte/store';
import type { EventRecord } from '../gen/EventRecord';
import type { Worktree } from '../gen/Worktree';

export interface GitFile {
  path: string;
  code: string;
  add: number;
  del: number;
}
export interface GitStatus {
  root: string;
  branch: string;
  files: GitFile[];
  worktree: Worktree | null;
  ahead?: number;
  committed?: Omit<GitFile, 'code'>[];
  merged?: boolean;
}
export interface TreeEntry {
  name: string;
  dir: boolean;
  size: number;
  dim: boolean;
}
export interface Tree {
  root: string;
  path: string;
  entries: TreeEntry[];
}
export interface FileText {
  path: string;
  rel: string;
  text: string;
}

/** A repository status, or null outside a repository (the engine answers `{}`). */
export function asStatus(value: unknown): GitStatus | null {
  if (!value || typeof value !== 'object' || !('branch' in value)) return null;
  return value as GitStatus;
}

/** Merge needs commits ahead of the base that are not merged yet. */
export function canMerge(status: GitStatus | null): boolean {
  return !!status?.worktree && (status.ahead ?? 0) > 0 && !status.merged;
}

/** "main · /repo" or "antiquota/x-ab12 from main · 2 ahead · merged". */
export function statusLine(status: GitStatus): string {
  if (!status.worktree) return `${status.branch} · ${status.root}`;
  const parts = [`${status.branch} from ${status.worktree.base}`, `${status.ahead ?? 0} ahead`];
  if (status.merged) parts.push('merged');
  return parts.join(' · ');
}

export function codeLabel(code: string): string {
  return (
    { '??': 'new', A: 'added', M: 'modified', D: 'deleted', R: 'renamed' }[code] ??
    code.toLowerCase()
  );
}

export interface EditHistory {
  file: string;
  diffs: string[];
  latest: number;
}

/** Diffs the agent's edits recorded, grouped by file, newest file first. */
export function editHistory(events: EventRecord[]): EditHistory[] {
  const byFile = new Map<string, EditHistory>();
  for (const event of events) {
    if (event.t !== 'tool' || !event.diff) continue;
    const params = (event.params ?? {}) as Record<string, unknown>;
    const file = String(params.TargetFile ?? params.File ?? params.AbsolutePath ?? event.name);
    const entry = byFile.get(file) ?? { file, diffs: [], latest: 0 };
    entry.diffs.push(event.diff);
    entry.latest = Math.max(entry.latest, event.ts);
    byFile.set(file, entry);
  }
  return [...byFile.values()].sort((a, b) => b.latest - a.latest);
}

/** Breadcrumb steps for a relative folder path: the root, then each folder. */
export function breadcrumbs(path: string): { label: string; path: string }[] {
  const parts = path.split('/').filter(Boolean);
  return [
    { label: 'project', path: '' },
    ...parts.map((label, index) => ({ label, path: parts.slice(0, index + 1).join('/') })),
  ];
}

export function parentPath(path: string): string {
  return path.split('/').filter(Boolean).slice(0, -1).join('/');
}

export function humanSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export type DrawerTab = 'changes' | 'files';
const KEY = 'antiquota.drawer';
function remembered(): { open: boolean; tab: DrawerTab } {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? '{}') as {
      open?: unknown;
      tab?: unknown;
    };
    return { open: value.open === true, tab: value.tab === 'files' ? 'files' : 'changes' };
  } catch {
    return { open: false, tab: 'changes' };
  }
}
/** The side drawer: whether it is open and which tab shows. Remembered per browser. */
export const drawer = writable(remembered());
drawer.subscribe((value) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(value));
  } catch {
    /* Storage can be unavailable; the drawer still works. */
  }
});

/** The file open in the viewer. */
export const viewer = writable<FileText | null>(null);
