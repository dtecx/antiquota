/** Owns readable tool labels, previews and grouped activity summaries. */
import type { EventRecord } from '../gen/EventRecord';
import { relativePath } from './approvals';

export type ToolEvent = Extract<EventRecord, { t: 'tool' }>;
export type ToolKind = 'run' | 'read' | 'list' | 'search' | 'edit' | 'web' | 'mcp' | 'other';
export type ToolInfo = { verb: string; arg: string; kind: ToolKind };

function parameters(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

export function toolInfo(event: ToolEvent, cwd: string): ToolInfo {
  const params = parameters(event.params);
  const field = (...keys: string[]) => {
    for (const key of keys)
      if (Object.hasOwn(params, key) && typeof params[key] === 'string' && params[key])
        return params[key] as string;
    return '';
  };
  const path = (...keys: string[]) => relativePath(field(...keys), cwd);
  switch (event.name) {
    case 'run_command':
      return { verb: 'Run', arg: field('CommandLine'), kind: 'run' };
    case 'view_file':
      return { verb: 'Read', arg: path('AbsolutePath'), kind: 'read' };
    case 'list_dir':
      return { verb: 'List', arg: path('DirectoryPath', 'SearchPath'), kind: 'list' };
    case 'grep_search':
      return { verb: 'Search', arg: field('Query'), kind: 'search' };
    case 'find_by_name':
      return { verb: 'Find', arg: field('Pattern'), kind: 'search' };
    case 'write_to_file':
      return { verb: 'Write', arg: path('TargetFile'), kind: 'edit' };
    case 'replace_file_content':
    case 'multi_replace_file_content':
      return { verb: 'Edit', arg: path('TargetFile'), kind: 'edit' };
    case 'sed_file':
      return { verb: 'Edit', arg: path('TargetFile', 'File'), kind: 'edit' };
    case 'read_url_content':
      return { verb: 'Fetch', arg: field('Url'), kind: 'web' };
    case 'search_web':
      return { verb: 'Web search', arg: field('query', 'Query'), kind: 'web' };
    case 'open_browser_url':
      return { verb: 'Browse', arg: field('Url'), kind: 'web' };
    case 'call_mcp_tool':
      return {
        verb: 'MCP',
        arg: [field('ServerName'), field('ToolName')].filter(Boolean).join(' · '),
        kind: 'mcp',
      };
    default:
      return {
        verb: event.name.replace(/_/g, ' '),
        arg:
          Object.values(params).find(
            (value): value is string => typeof value === 'string' && value.length < 300,
          ) ?? '',
        kind: 'other',
      };
  }
}

function diffStats(diff: string): { add: number; del: number } {
  let add = 0,
    del = 0;
  for (const line of diff.split('\n')) {
    if (line.startsWith('+') && !line.startsWith('+++')) add++;
    else if (line.startsWith('-') && !line.startsWith('---')) del++;
  }
  return { add, del };
}

export type ToolPreview = { text: string; more: number; kind: 'output' | 'diff' };
const colors = new RegExp(`${String.fromCharCode(27)}\\[[0-?]*[ -/]*[@-~]`, 'g');

function toolState(event: ToolEvent) {
  const error = event.error !== undefined && event.error !== null && event.error !== '';
  return error || ['error', 'failed', 'failure'].includes(event.state.toLowerCase())
    ? 'error'
    : ['active', 'running'].includes(event.state.toLowerCase())
      ? 'running'
      : 'done';
}

export function toolView(event: ToolEvent, cwd: string) {
  const output = (event.output ?? '').replace(colors, '');
  const diff = event.diff ?? '';
  const error =
    event.error === undefined || event.error === null
      ? ''
      : typeof event.error === 'string'
        ? event.error
        : JSON.stringify(event.error, null, 2);
  const state = toolState(event);
  let preview: ToolPreview | null = null;
  if (state !== 'running') {
    if (diff) {
      const lines = diff
        .split('\n')
        .filter((line) => /^[+-]/.test(line) && !/^(\+\+\+|---)/.test(line));
      if (lines.length)
        preview = {
          text: lines.slice(0, 6).join('\n'),
          more: Math.max(0, lines.length - 6),
          kind: 'diff',
        };
    } else if (output.trimEnd() && event.name === 'run_command') {
      const lines = output.trimEnd().split('\n');
      preview = {
        text: lines.slice(-4).join('\n'),
        more: Math.max(0, lines.length - 4),
        kind: 'output',
      };
    }
  }
  const body = diff
    ? { text: diff, kind: 'diff' as const, error }
    : output
      ? { text: output, kind: 'output' as const, error }
      : { text: JSON.stringify(event.params, null, 2), kind: 'params' as const, error };
  return {
    ...toolInfo(event, cwd),
    state,
    secs: event.secs,
    output,
    diff,
    error,
    ...diffStats(diff),
    preview,
    body,
  };
}

const labels: Record<ToolKind, [string, string, string]> = {
  run: ['Ran', 'command', 'commands'],
  read: ['Read', 'file', 'files'],
  edit: ['Edited', 'file', 'files'],
  list: ['Listed', 'folder', 'folders'],
  search: ['Searched', 'time', 'times'],
  web: ['Fetched', 'page', 'pages'],
  mcp: ['Called', 'MCP tool', 'MCP tools'],
  other: ['Used', 'tool', 'tools'],
};
const order: ToolKind[] = ['edit', 'run', 'read', 'search', 'list', 'web', 'mcp', 'other'];

export function activitySummary(events: readonly EventRecord[], cwd: string) {
  const counts: Record<ToolKind, number> = {
    run: 0,
    read: 0,
    edit: 0,
    list: 0,
    search: 0,
    web: 0,
    mcp: 0,
    other: 0,
  };
  const files = { read: new Set<string>(), edit: new Set<string>() };
  let add = 0,
    del = 0,
    failed = 0,
    steps = 0;
  let running: ToolEvent | null = null;
  for (const event of events) {
    if (event.t !== 'tool') continue;
    const info = toolInfo(event, cwd),
      stats = diffStats(event.diff ?? ''),
      state = toolState(event);
    if (info.kind === 'read' || info.kind === 'edit') files[info.kind].add(info.arg);
    else counts[info.kind]++;
    add += stats.add;
    del += stats.del;
    failed += Number(state === 'error');
    steps++;
    if (state === 'running') running = event;
  }
  counts.read = files.read.size;
  counts.edit = files.edit.size;
  const parts = order
    .filter((kind) => counts[kind])
    .map((kind, index) => {
      const [verb, one, many] = labels[kind];
      return `${index ? verb.toLowerCase() : verb} ${counts[kind]} ${counts[kind] === 1 ? one : many}`;
    });
  const current = running && toolInfo(running, cwd);
  const label = current ? `${current.verb} ${current.arg}`.trimEnd() + '…' : parts.join(', ');
  return { label, counts, add, del, failed, steps, running };
}
