/** Owns typed notice presentation without interpreting text or inventing permission rules. */
import type { EventRecord } from '../gen/EventRecord';

export type NoticeEvent = Extract<EventRecord, { t: 'notice' }>;
export type NoticeTone = 'info' | 'ok' | 'bad' | 'warning';
export type DeniedAction = { action: string; display_name?: string };

// Matches the file-changing tools in core policy; accepting edits cannot permit commands.
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
const COMMAND_TOOLS = new Set([
  'run_command',
  'shell_exec',
  'send_command_input',
  'execute_notebook',
  'run_extension_code',
  'restart_dev_server',
  'git_commit',
  'run_in_background',
]);

function object(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}
function own(value: Record<string, unknown>, key: string): unknown {
  return Object.hasOwn(value, key) ? value[key] : undefined;
}
function text(value: unknown): string {
  return typeof value === 'string' && value.trim() ? value : '';
}
function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => Boolean(text(item))) : [];
}

export function noticeView(event: NoticeEvent) {
  const data = object(event.data);
  const field = (key: string) => own(data, key);
  let title = '',
    tone: NoticeTone = 'info';
  switch (event.kind) {
    case 'handoff':
      title = 'Account handoff';
      break;
    case 'wall':
      title = 'Out of quota';
      tone = 'warning';
      break;
    case 'noaccount':
      title = 'No account';
      tone = 'warning';
      break;
    case 'goal':
      title = field('met') === true ? 'Goal met' : 'Goal not met';
      tone = field('met') === true ? 'ok' : 'info';
      break;
    case 'check':
      title = field('passed') === true ? 'Check passed' : 'Check failed';
      tone = field('passed') === true ? 'ok' : 'bad';
      break;
    case 'verdict':
      title =
        field('done') === true
          ? 'Verifier · done'
          : field('done') === false
            ? 'Verifier · not met'
            : 'Verifier · indeterminate';
      tone = field('done') === true ? 'ok' : field('done') === false ? 'bad' : 'info';
      break;
    case 'denied':
      title = 'Permission denied';
      tone = 'warning';
      break;
    case 'error':
      title = 'Error';
      tone = 'bad';
      break;
    case 'rewind':
      title = 'Rewind';
      break;
  }
  const commands = strings(field('commands'));
  const actions: DeniedAction[] = Array.isArray(field('actions'))
    ? (field('actions') as unknown[]).flatMap((value) => {
        const action = object(value),
          name = text(own(action, 'action')),
          display = text(own(action, 'display_name'));
        return name ? [{ action: name, ...(display ? { display_name: display } : {}) }] : [];
      })
    : [];
  const outside = new Set(strings(field('outside')));
  const files = [...new Set([...strings(field('files')), ...outside])].map((path) => ({
    path,
    outside: outside.has(path),
  }));
  const commandsEmpty =
    field('commands') === undefined ||
    (Array.isArray(field('commands')) && (field('commands') as unknown[]).length === 0);
  return {
    title,
    tone,
    output: text(field('output')),
    missing: strings(field('missing')),
    identity: [text(field('slot')), text(field('model'))].filter(Boolean).join(' · '),
    files,
    commands,
    rules: event.kind === 'denied' ? [...new Set(strings(field('rules')))] : [],
    actions,
    canAcceptEdits:
      event.kind === 'denied' &&
      commandsEmpty &&
      actions.some(({ action }) => WRITE_TOOLS.has(action)) &&
      !actions.some(({ action }) => COMMAND_TOOLS.has(action)),
  };
}
