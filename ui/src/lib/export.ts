/** Owns durable Markdown transcript export and browser download cleanup. */
import type { EventRecord } from '../gen/EventRecord';
import type { Session } from '../gen/Session';
import { relativePath } from './approvals';
import { projectHistory } from './transcript';

const speakers: Record<string, string> = {
  user: 'you',
  message: 'you',
  goal: 'goal',
  nudge: 'goal push',
  job: 'job',
  loop: 'loop',
};
const toolFields: Record<string, string[]> = {
  run_command: ['CommandLine'],
  view_file: ['AbsolutePath'],
  list_dir: ['DirectoryPath', 'SearchPath'],
  grep_search: ['Query'],
  find_by_name: ['Pattern'],
  write_to_file: ['TargetFile'],
  replace_file_content: ['TargetFile'],
  multi_replace_file_content: ['TargetFile'],
  sed_file: ['TargetFile', 'File'],
  read_url_content: ['Url'],
  search_web: ['query', 'Query'],
  open_browser_url: ['Url'],
};
const pathTools = new Set([
  'view_file',
  'list_dir',
  'write_to_file',
  'replace_file_content',
  'multi_replace_file_content',
  'sed_file',
]);

function toolDetail(event: Extract<EventRecord, { t: 'tool' }>, cwd: string): string {
  if (!event.params || typeof event.params !== 'object' || Array.isArray(event.params)) return '';
  const params = event.params;
  if (event.name === 'call_mcp_tool')
    return [params.ServerName, params.ToolName]
      .filter((value) => typeof value === 'string')
      .join(' · ');
  const fields = Object.hasOwn(toolFields, event.name) ? toolFields[event.name] : undefined;
  const detail = fields
    ? fields.map((key) => params[key]).find((value) => typeof value === 'string' && value)
    : Object.values(params).find((value) => typeof value === 'string' && value.length < 300);
  const text = typeof detail === 'string' ? detail : '';
  return pathTools.has(event.name) ? relativePath(text, cwd) : text;
}

export function sessionMarkdown(
  session: Session,
  events: EventRecord[],
): { filename: string; text: string } {
  const out = [`# ${session.title || 'Untitled'}`, '', `${session.cwd} · ${session.model}`, ''];
  for (const event of projectHistory(events)) {
    switch (event.t) {
      case 'user':
        out.push(
          `## ${Object.hasOwn(speakers, event.kind) ? speakers[event.kind] : 'you'}`,
          '',
          event.text,
          '',
        );
        break;
      case 'assistant':
        out.push(event.text, '');
        break;
      case 'thinking':
        out.push('<details><summary>Thought</summary>', '', event.text, '', '</details>', '');
        break;
      case 'tool': {
        const detail = toolDetail(event, session.cwd);
        out.push(
          `- \`${event.name}\`${detail ? ` ${detail}` : ''}${event.state.toLowerCase() === 'error' ? ' (error)' : ''}`,
        );
        break;
      }
      case 'turn_start':
        out.push('', '---', `*turn ${event.turn} · ${event.slot} · ${event.model}*`, '');
        break;
      case 'turn_end':
        out.push(`*turn ${event.turn} ended · ${event.status}*`, '');
        break;
      case 'notice':
        out.push(`> **${event.kind}**: ${event.text}`, '');
        break;
    }
  }
  return {
    filename: `${(session.title || 'session').replace(/[^\w.-]+/g, '-').slice(0, 60)}.md`,
    text: `${out.join('\n')}\n`,
  };
}

export function downloadSession(session: Session, events: EventRecord[]): void {
  const { filename, text } = sessionMarkdown(session, events);
  const anchor = document.createElement('a');
  const url = URL.createObjectURL(new Blob([text], { type: 'text/markdown;charset=utf-8' }));
  try {
    anchor.href = url;
    anchor.download = filename;
    document.body.append(anchor);
    anchor.click();
  } finally {
    anchor.remove();
    // Give the browser time to consume the download URL before releasing it.
    setTimeout(() => URL.revokeObjectURL(url), 0);
  }
}
