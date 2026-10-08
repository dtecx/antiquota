/** Proves notice data stays literal and permission choices never broaden command access. */
import { describe, expect, it } from 'vitest';
import { noticeView, type NoticeEvent } from './notices';

function notice(
  kind: string,
  data: NoticeEvent['data'] = null,
  text = 'Literal notice text',
): NoticeEvent {
  return { t: 'notice', ts: 1, kind, text, data };
}

describe('notice presentation', () => {
  it.each([
    ['handoff', {}, 'Account handoff', 'info'],
    ['wall', {}, 'Out of quota', 'warning'],
    ['noaccount', {}, 'No account', 'warning'],
    ['goal', { met: true }, 'Goal met', 'ok'],
    ['goal', { met: false }, 'Goal not met', 'info'],
    ['goal', { met: 'true' }, 'Goal not met', 'info'],
    ['check', { passed: true }, 'Check passed', 'ok'],
    ['check', { passed: false }, 'Check failed', 'bad'],
    ['check', { passed: 1 }, 'Check failed', 'bad'],
    ['verdict', { done: true }, 'Verifier · done', 'ok'],
    ['verdict', { done: false }, 'Verifier · not met', 'bad'],
    ['verdict', { done: null }, 'Verifier · indeterminate', 'info'],
    ['verdict', { done: 'false' }, 'Verifier · indeterminate', 'info'],
    ['verdict', { done: 1 }, 'Verifier · indeterminate', 'info'],
    ['verdict', {}, 'Verifier · indeterminate', 'info'],
    ['denied', {}, 'Permission denied', 'warning'],
    ['error', {}, 'Error', 'bad'],
    ['rewind', {}, 'Rewind', 'info'],
    ['future_notice', {}, '', 'info'],
    ['constructor', {}, '', 'info'],
  ] as const)('presents %s with strict typed state', (kind, data, title, tone) => {
    expect(noticeView(notice(kind, data))).toMatchObject({ title, tone });
  });

  it('preserves check output and only actual missing-work and account/model strings', () => {
    expect(
      noticeView(notice('check', { passed: false, output: '<script>literal</script>\nfailed' }))
        .output,
    ).toBe('<script>literal</script>\nfailed');
    expect(
      noticeView(
        notice('verdict', {
          done: false,
          missing: ['finish tests', null, false, 12, {}, ''],
          slot: 'synthetic@example.test',
          model: 'gemini-fixture',
        }),
      ),
    ).toMatchObject({
      missing: ['finish tests'],
      identity: 'synthetic@example.test · gemini-fixture',
    });
    expect(
      noticeView(notice('verdict', { slot: 'synthetic@example.test', model: null })).identity,
    ).toBe('synthetic@example.test');
    expect(noticeView(notice('verdict', { slot: false, model: 'gemini-fixture' })).identity).toBe(
      'gemini-fixture',
    );
    expect(noticeView(notice('verdict', { slot: 12, model: [] })).identity).toBe('');
  });

  it('leaves error text untouched for literal component rendering', () => {
    const event = notice('error', {}, '<img src=x onerror=alert(1)>\n& **unparsed**');
    const before = structuredClone(event);
    expect(noticeView(event)).toMatchObject({
      title: 'Error',
      tone: 'bad',
      output: '',
      rules: [],
      canAcceptEdits: false,
    });
    expect(event).toEqual(before);
    expect(event.text).toBe('<img src=x onerror=alert(1)>\n& **unparsed**');
  });

  it('rejects malformed containers and scalar or nested presentation fields', () => {
    for (const data of [
      null,
      false,
      true,
      0,
      12,
      'scalar',
      ['array'],
      {
        output: false,
        missing: 'wrong',
        slot: {},
        model: false,
        files: 12,
        outside: 'wrong',
        commands: false,
        rules: 'command(ls)',
        actions: 'write_to_file',
      },
    ]) {
      expect(noticeView(notice('denied', data))).toMatchObject({
        output: '',
        missing: [],
        identity: '',
        files: [],
        commands: [],
        rules: [],
        actions: [],
        canAcceptEdits: false,
      });
    }
    const inherited = Object.create({
      commands: [],
      actions: [{ action: 'write_to_file' }],
      rules: ['command(ls)'],
      slot: 'inherited',
      output: 'inherited',
    });
    expect(noticeView(notice('denied', inherited))).toMatchObject({
      rules: [],
      actions: [],
      output: '',
      identity: '',
      canAcceptEdits: false,
    });
  });

  it('marks historical rewind paths changed outside the agent including outside-only paths', () => {
    const event = notice('rewind', {
      files: ['restored.rs', 'user-edited.rs', null, 'restored.rs'],
      outside: ['user-edited.rs', 'outside-only.rs', false],
    });
    const before = structuredClone(event);
    expect(noticeView(event).files).toEqual([
      { path: 'restored.rs', outside: false },
      { path: 'user-edited.rs', outside: true },
      { path: 'outside-only.rs', outside: true },
    ]);
    expect(event).toEqual(before);
  });
});

describe('denied notice choices', () => {
  it('uses only explicit backend-vetted rules and preserves valid command/action names', () => {
    const view = noticeView(
      notice('denied', {
        commands: ['ls project', null, false, ''],
        rules: ['command(ls)', 'command(ls)', null, {}, ''],
        actions: [{ action: 'run_command', display_name: 'Run command' }, { action: false }, null],
      }),
    );
    expect(view).toMatchObject({
      commands: ['ls project'],
      rules: ['command(ls)'],
      actions: [{ action: 'run_command', display_name: 'Run command' }],
      canAcceptEdits: false,
    });
    expect(noticeView(notice('denied', { commands: ['ls project'] })).rules).toEqual([]);
    expect(noticeView(notice('info', { rules: ['command(ls)'] })).rules).toEqual([]);
  });

  it.each([
    'write_to_file',
    'replace_file_content',
    'multi_replace_file_content',
    'sed_file',
    'file_change',
    'edit_notebook',
    'move',
    'delete_directory',
    'write_blob',
  ])('offers accept edits for a command-free %s action', (action) => {
    expect(
      noticeView(notice('denied', { commands: [], actions: [{ action }] })).canAcceptEdits,
    ).toBe(true);
    expect(
      noticeView(notice('denied', { actions: [{ action, display_name: false }] })),
    ).toMatchObject({ actions: [{ action }], canAcceptEdits: true });
  });

  it.each([
    'python -c "delete()"',
    'node -e "delete()"',
    'sh -c "anything"',
    'env python script.py',
    'find . -exec sh -c anything \\;',
    'xargs rm',
    'awk "system(command)"',
  ])('never invents a command rule or edit retry for %s', (command) => {
    expect(
      noticeView(notice('denied', { commands: [command], actions: [{ action: 'write_to_file' }] })),
    ).toMatchObject({ rules: [], canAcceptEdits: false });
  });

  it('does not offer edit retry for unknown actions runners or malformed command lists', () => {
    for (const action of [
      'unknown_tool',
      'call_mcp_tool',
      'run_command',
      'shell_exec',
      'send_command_input',
      'execute_notebook',
      'run_extension_code',
      'restart_dev_server',
      'git_commit',
      'run_in_background',
    ])
      expect(
        noticeView(notice('denied', { commands: [], actions: [{ action }] })).canAcceptEdits,
      ).toBe(false);
    expect(
      noticeView(
        notice('denied', {
          commands: [],
          actions: [{ action: 'write_to_file' }, { action: 'run_command' }],
        }),
      ).canAcceptEdits,
    ).toBe(false);
    for (const commands of [null, false, 12, 'command', [null], [false], [{}]])
      expect(
        noticeView(notice('denied', { commands, actions: [{ action: 'write_to_file' }] }))
          .canAcceptEdits,
      ).toBe(false);
    expect(noticeView(notice('denied', { commands: [], actions: [] })).canAcceptEdits).toBe(false);
    expect(
      noticeView(notice('error', { commands: [], actions: [{ action: 'write_to_file' }] }))
        .canAcceptEdits,
    ).toBe(false);
  });
});
