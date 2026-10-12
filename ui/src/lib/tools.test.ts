/** Proves tool presentation preserves complete content and truthful activity counts. */
import { describe, expect, it } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import { activitySummary, toolInfo, toolView, type ToolEvent } from './tools';

const cwd = '/Desktop/test/project';
function tool(patch: Partial<ToolEvent> = {}): ToolEvent {
  return {
    t: 'tool',
    ts: 1,
    turn: 1,
    step: 1,
    name: 'run_command',
    params: {},
    state: 'done',
    ...patch,
  };
}

describe('tool presentation', () => {
  it.each([
    ['run_command', { CommandLine: 'cargo test' }, 'Run', 'cargo test', 'run'],
    ['view_file', { AbsolutePath: `${cwd}/src/main.rs` }, 'Read', 'src/main.rs', 'read'],
    ['list_dir', { DirectoryPath: `${cwd}/src` }, 'List', 'src', 'list'],
    ['list_dir', { SearchPath: `${cwd}/tests` }, 'List', 'tests', 'list'],
    ['grep_search', { Query: 'needle' }, 'Search', 'needle', 'search'],
    ['find_by_name', { Pattern: '*.rs' }, 'Find', '*.rs', 'search'],
    ['write_to_file', { TargetFile: `${cwd}/new.rs` }, 'Write', 'new.rs', 'edit'],
    ['replace_file_content', { TargetFile: `${cwd}/main.rs` }, 'Edit', 'main.rs', 'edit'],
    ['multi_replace_file_content', { TargetFile: `${cwd}/main.rs` }, 'Edit', 'main.rs', 'edit'],
    ['sed_file', { File: `${cwd}/main.rs` }, 'Edit', 'main.rs', 'edit'],
    [
      'sed_file',
      { TargetFile: `${cwd}/chosen.rs`, File: `${cwd}/ignored.rs` },
      'Edit',
      'chosen.rs',
      'edit',
    ],
    [
      'read_url_content',
      { Url: 'https://example.test/read' },
      'Fetch',
      'https://example.test/read',
      'web',
    ],
    [
      'search_web',
      { query: 'lowercase query', Query: 'ignored query' },
      'Web search',
      'lowercase query',
      'web',
    ],
    ['search_web', { Query: 'uppercase query' }, 'Web search', 'uppercase query', 'web'],
    ['open_browser_url', { Url: 'https://example.test' }, 'Browse', 'https://example.test', 'web'],
    [
      'call_mcp_tool',
      { ServerName: 'synthetic', ToolName: 'lookup' },
      'MCP',
      'synthetic · lookup',
      'mcp',
    ],
  ] as const)('labels %s with its relevant argument', (name, params, verb, arg, kind) => {
    expect(toolInfo(tool({ name, params }), cwd)).toEqual({ verb, arg, kind });
  });

  it('keeps paths outside the project absolute and honors a trailing project slash', () => {
    const event = tool({ name: 'view_file', params: { AbsolutePath: `${cwd}-other/file.rs` } });
    expect(toolInfo(event, cwd).arg).toBe(`${cwd}-other/file.rs`);
    expect(toolInfo({ ...event, params: { AbsolutePath: `${cwd}/file.rs` } }, `${cwd}/`).arg).toBe(
      'file.rs',
    );
  });

  it('uses readable unknown names and only string arguments shorter than 300 characters', () => {
    const params = {
      nested: { ignored: 'nested' },
      tooLong: 'x'.repeat(300),
      first: 'x'.repeat(299),
      second: 'short',
    };
    expect(toolInfo(tool({ name: 'unknown_tool', params }), cwd)).toEqual({
      verb: 'unknown tool',
      arg: 'x'.repeat(299),
      kind: 'other',
    });
    for (const name of ['constructor', 'toString', '__proto__'])
      expect(toolInfo(tool({ name, params: { value: 'safe' } }), cwd)).toEqual({
        verb: name.replace(/_/g, ' '),
        arg: 'safe',
        kind: 'other',
      });
  });

  it('ignores malformed parameters and inherited fields without crashing', () => {
    for (const params of [null, [], 'not an object', 42, false, { AbsolutePath: { nested: true } }])
      expect(toolInfo(tool({ name: 'view_file', params }), cwd).arg).toBe('');
    const inherited = Object.create({
      CommandLine: 'inherited dangerous command',
      detail: 'inherited string',
    });
    expect(toolInfo(tool({ params: inherited }), cwd).arg).toBe('');
    expect(toolInfo(tool({ name: 'unknown_tool', params: inherited }), cwd).arg).toBe('');
    expect(
      toolInfo(tool({ name: 'call_mcp_tool', params: { ServerName: {}, ToolName: 'lookup' } }), cwd)
        .arg,
    ).toBe('lookup');
    expect(
      toolInfo(
        tool({
          params: JSON.parse('{"__proto__":{"CommandLine":"ignored"},"CommandLine":"owned"}'),
        }),
        cwd,
      ).arg,
    ).toBe('owned');
  });

  it('normalizes running completed and failure states while retaining zero and fractional seconds', () => {
    for (const state of ['active', 'ACTIVE', 'running'])
      expect(toolView(tool({ state, secs: 0 }), cwd)).toMatchObject({
        state: 'running',
        secs: 0,
        preview: null,
      });
    for (const state of ['done', 'DONE', 'SUCCESS', 'completed'])
      expect(toolView(tool({ state, secs: 1.25 }), cwd)).toMatchObject({
        state: 'done',
        secs: 1.25,
      });
    for (const state of ['error', 'ERROR', 'failed'])
      expect(toolView(tool({ state }), cwd).state).toBe('error');
    expect(toolView(tool({ state: 'DONE', error: 'Failure' }), cwd).state).toBe('error');
    expect(toolView(tool(), cwd).secs).toBeUndefined();
  });

  it('strips terminal sequences and previews the last four trimmed command output lines', () => {
    const event = tool({ output: '\x1b[31mone\x1b[0m\ntwo\nthree\nfour\nfive\nsix\n  \n' });
    const before = structuredClone(event);
    const view = toolView(event, cwd);
    expect(view.output).toBe('one\ntwo\nthree\nfour\nfive\nsix\n  \n');
    expect(view.preview).toEqual({ text: 'three\nfour\nfive\nsix', more: 2, kind: 'output' });
    expect(view.body).toEqual({ text: view.output, kind: 'output', error: '' });
    expect(event).toEqual(before);
    expect(toolView(tool({ output: 'only line\n' }), cwd).preview).toEqual({
      text: 'only line',
      more: 0,
      kind: 'output',
    });
    expect(toolView(tool({ output: ' \n' }), cwd).preview).toBeNull();
    expect(toolView(tool({ name: 'view_file', output: 'file contents' }), cwd).preview).toBeNull();
  });

  it('previews six changed diff lines and counts changes without headers or context', () => {
    const diff =
      '--- a/file\n+++ b/file\n@@ -1,4 +1,4 @@\n context\n-old1\n+new1\n-old2\n+new2\n-old3\n+new3\n-old4\n+new4\n';
    const view = toolView(
      tool({ name: 'replace_file_content', diff, output: 'ignored output' }),
      cwd,
    );
    expect(view).toMatchObject({ add: 4, del: 4 });
    expect(view.preview).toEqual({
      text: '-old1\n+new1\n-old2\n+new2\n-old3\n+new3',
      more: 2,
      kind: 'diff',
    });
    expect(view.body).toEqual({ text: diff, kind: 'diff', error: '' });
    expect(
      toolView(tool({ diff: '--- a/file\n+++ b/file\n@@ unchanged @@\n context' }), cwd).preview,
    ).toBeNull();
    expect(toolView(tool({ diff: '-old\n+new' }), cwd).preview?.more).toBe(0);
  });

  it('keeps running output and diffs in the full body without showing a preview', () => {
    for (const patch of [{ output: 'one\ntwo\nthree\nfour\nfive' }, { diff: '-old\n+new' }]) {
      const view = toolView(tool({ ...patch, state: 'ACTIVE' }), cwd);
      expect(view.preview).toBeNull();
      expect(view.body.text).not.toBe('');
    }
  });

  it('retains all errors and serializes parameters only when no diff or output exists', () => {
    const error = 'First line\nSecond line\nThird line\nFourth line';
    const params = { CommandLine: 'broken', Arguments: { nested: 'detail' } };
    const view = toolView(tool({ params, error }), cwd);
    expect(view.body).toEqual({ text: JSON.stringify(params, null, 2), kind: 'params', error });
    expect(view.error).toBe(error);
    expect(toolView(tool({ output: 'partial output', params, error }), cwd).body).toEqual({
      text: 'partial output',
      kind: 'output',
      error,
    });
    const structuredError = { code: 7, details: ['first', 'second'] };
    expect(toolView(tool({ params: null, error: structuredError }), cwd)).toMatchObject({
      error: JSON.stringify(structuredError, null, 2),
      body: { text: 'null', kind: 'params', error: JSON.stringify(structuredError, null, 2) },
    });
  });
});

describe('grouped activity', () => {
  it('counts unique read and edit paths with ordered labels and all tool steps', () => {
    const events: EventRecord[] = [
      tool({ name: 'view_file', params: { AbsolutePath: `${cwd}/file.rs` } }),
      tool({ name: 'view_file', params: { AbsolutePath: 'file.rs' } }),
      tool({
        name: 'write_to_file',
        params: { TargetFile: `${cwd}/file.rs` },
        diff: '--- a\n+++ b\n-old\n+new',
      }),
      tool({
        name: 'sed_file',
        params: { File: `${cwd}/file.rs` },
        diff: '+another',
        state: 'ERROR',
      }),
      tool(),
      tool({ name: 'grep_search' }),
      tool({ name: 'find_by_name' }),
      tool({ name: 'list_dir' }),
      tool({ name: 'read_url_content' }),
      tool({ name: 'call_mcp_tool' }),
      tool({ name: 'other_tool' }),
      { t: 'thinking', ts: 2, turn: 1, step: 12, text: 'not a tool', tokens: 100 },
      { t: 'assistant', ts: 3, turn: 1, step: 13, text: 'not a tool either' },
    ];
    const before = structuredClone(events);
    expect(activitySummary(events, cwd)).toEqual({
      label:
        'Edited 1 file, ran 1 command, read 1 file, searched 2 times, listed 1 folder, fetched 1 page, called 1 MCP tool, used 1 tool',
      counts: { skill: 0, edit: 1, run: 1, read: 1, search: 2, list: 1, web: 1, mcp: 1, other: 1 },
      add: 2,
      del: 1,
      failed: 1,
      steps: 11,
      running: null,
    });
    expect(events).toEqual(before);
  });

  it('names loaded skills first, from file reads or plain shell reads only', () => {
    const skill = (path: string) => tool({ name: 'view_file', params: { AbsolutePath: path } });
    expect(toolInfo(skill(`${cwd}/.agents/skills/caveman/SKILL.md`), cwd)).toEqual({
      verb: 'Load skill',
      arg: 'caveman',
      kind: 'skill',
    });
    expect(
      toolInfo(tool({ params: { CommandLine: "cat '/home/u/skills/ponytail/SKILL.md'" } }), cwd),
    ).toMatchObject({ kind: 'skill', arg: 'ponytail' });
    expect(toolInfo(tool({ params: { CommandLine: 'cat a/SKILL.md | sh' } }), cwd).kind).toBe(
      'run',
    );
    expect(toolInfo(skill(`${cwd}/docs/SKILL.md.bak`), cwd).kind).toBe('read');
    expect(
      activitySummary(
        [
          skill('/s/caveman/SKILL.md'),
          tool(),
          skill('/s/ponytail/SKILL.md'),
          skill('/s/caveman/SKILL.md'),
        ],
        cwd,
      ).label,
    ).toBe('Loaded skills caveman, ponytail, ran 1 command');
  });

  it('uses plural labels and the most recent running tool instead of completed counts', () => {
    const first = tool({ params: { CommandLine: 'first command' }, state: 'running' });
    const current = tool({
      name: 'view_file',
      params: { AbsolutePath: `${cwd}/current.rs` },
      state: 'ACTIVE',
    });
    expect(activitySummary([first, tool(), current], cwd)).toMatchObject({
      label: 'Read current.rs…',
      steps: 3,
      running: current,
    });
    expect(activitySummary([tool(), tool()], cwd).label).toBe('Ran 2 commands');
    expect(
      activitySummary(
        [
          tool({ name: 'view_file', params: { AbsolutePath: 'one' } }),
          tool({ name: 'view_file', params: { AbsolutePath: 'two' } }),
        ],
        cwd,
      ).label,
    ).toBe('Read 2 files');
    expect(activitySummary([tool({ name: 'constructor', state: 'running' })], cwd).label).toBe(
      'constructor…',
    );
  });

  it('does not count thinking as tools and reports payload failures even with a completed state', () => {
    expect(
      activitySummary(
        [{ t: 'thinking', ts: 1, turn: 1, step: 1, text: 'Thought', tokens: 12 }],
        cwd,
      ),
    ).toMatchObject({ label: '', steps: 0, failed: 0, running: null });
    expect(
      activitySummary([tool({ state: 'done', error: { message: 'failed' } })], cwd),
    ).toMatchObject({ label: 'Ran 1 command', failed: 1, running: null });
  });
});
