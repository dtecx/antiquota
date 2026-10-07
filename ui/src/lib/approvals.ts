/** Owns how an approval request reads: its target, preview, wait and answer choices. */
import type { EventRecord } from '../gen/EventRecord';

export type ApprovalEvent = Extract<EventRecord, { t: 'approval' }>;

/** File-changing tools; "always" on one of them lets the session make edits. Mirrors core policy. */
const WRITE_TOOLS = new Set([
  'write_to_file',
  'replace_file_content',
  'multi_replace_file_content',
  'sed_file',
  'file_change',
  'edit_notebook',
  'move',
  'delete_directory',
  'write_blob',
]);
const PREVIEW_LINES = 14;

function field(args: unknown, ...keys: string[]): string {
  if (!args || typeof args !== 'object') return '';
  for (const key of keys) {
    const value = (args as Record<string, unknown>)[key];
    if (typeof value === 'string' && value) return value;
  }
  return '';
}

export function relativePath(path: string, cwd: string): string {
  const base = cwd.replace(/[\\/]+$/, '');
  if (base && (path.startsWith(`${base}/`) || path.startsWith(`${base}\\`)))
    return path.slice(base.length + 1);
  return path;
}

export type ApprovalKind = 'command' | 'edit' | 'mcp' | 'tool';

export function approvalKind(event: ApprovalEvent): ApprovalKind {
  if (['run_command', 'git_commit', 'run_in_background'].includes(event.tool)) return 'command';
  if (WRITE_TOOLS.has(event.tool)) return 'edit';
  if (field(event.args, 'ServerName', 'server')) return 'mcp';
  return 'tool';
}

/** The command, file, or MCP tool the request is about. */
export function approvalTarget(event: ApprovalEvent, cwd: string): string {
  switch (approvalKind(event)) {
    case 'command':
      return field(event.args, 'CommandLine') || (event.tool === 'git_commit' ? 'git commit' : '');
    case 'edit':
      return relativePath(
        field(event.args, 'TargetFile', 'File', 'AbsolutePath', 'Path', 'Source'),
        cwd,
      );
    case 'mcp':
      return `${field(event.args, 'ServerName', 'server')}/${field(event.args, 'ToolName', 'tool')}`;
    default:
      return event.tool;
  }
}

export function approvalTitle(event: ApprovalEvent, cwd: string): string {
  const target = approvalTarget(event, cwd);
  switch (approvalKind(event)) {
    case 'command':
      return event.tool === 'run_in_background'
        ? 'agy wants to start a background job'
        : 'agy wants to run a command';
    case 'edit':
      return target ? `agy wants to edit ${target}` : 'agy wants to change files';
    case 'mcp':
      return `agy wants to call ${target}`;
    default:
      return `agy wants to use ${event.tool}`;
  }
}

/** Text worth reading before answering: the command, the new file text, or the call input. */
export function approvalPreview(event: ApprovalEvent): string {
  const head = (text: string) => text.split('\n').slice(0, PREVIEW_LINES).join('\n');
  switch (approvalKind(event)) {
    case 'command':
      return field(event.args, 'CommandLine');
    case 'edit':
      return head(field(event.args, 'CodeContent', 'ReplacementContent', 'Content'));
    default: {
      const input =
        event.args && typeof event.args === 'object'
          ? ((event.args as Record<string, unknown>).Arguments ?? event.args)
          : event.args;
      return input && JSON.stringify(input) !== '{}' ? head(JSON.stringify(input, null, 2)) : '';
    }
  }
}

/** Whole minutes the request waits before agy hears "no". */
export function waitMinutes(event: ApprovalEvent): number {
  return Math.max(0, Math.round((event.deadline - event.ts) / 60));
}

/** The "always" choice: the permanent rule, or edits for this session, or none. */
export function alwaysChoice(event: ApprovalEvent): { label: string; title: string } | null {
  if (event.rule)
    return {
      label: `Always allow ${event.rule}`,
      title: "Added to antiquota's allow rules for every session",
    };
  if (WRITE_TOOLS.has(event.tool))
    return {
      label: 'Allow all edits in this session',
      title: 'Switches this session to accept edits',
    };
  return null;
}

/** How an answered request reads in the transcript. */
export function answerSummary(event: ApprovalEvent): string {
  const always = (event as { always?: unknown }).always === true;
  if (event.state === 'allowed') {
    if (!always) return 'once';
    return event.rule ? `always: ${event.rule}` : 'edits allowed for this session';
  }
  return event.why || 'by you';
}
