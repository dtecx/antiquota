/** Proves exported transcript content, safe filenames, and browser download cleanup. */
import { afterEach, expect, it, vi } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import { fixtureSession } from './fixtures.test-support';
import { downloadSession, sessionMarkdown } from './export';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

const user = (text: string, kind = 'user'): EventRecord => ({ t: 'user', ts: 1, text, kind });
const tool = (
  params: Extract<EventRecord, { t: 'tool' }>['params'],
): Extract<EventRecord, { t: 'tool' }> => ({
  t: 'tool',
  ts: 2,
  turn: 1,
  step: 1,
  name: 'run_command',
  params,
  state: 'done',
});

it('exports durable prompts, replies, reasoning, tool calls, turn boundaries and notices', () => {
  const session = fixtureSession();
  const events: EventRecord[] = [
    user('What does this code do?'),
    { t: 'turn_start', ts: 2, turn: 1, slot: 'synthetic@example.test', model: session.model },
    { t: 'thinking', ts: 3, turn: 1, step: 1, text: 'Read the source first.', tokens: 12 },
    { ...tool({ CommandLine: 'cargo test' }), ts: 4, step: 2, state: 'ERROR' },
    { ...tool({ AbsolutePath: `${session.cwd}/src/main.rs` }), ts: 5, step: 3, name: 'view_file' },
    {
      t: 'assistant',
      ts: 6,
      turn: 1,
      step: 4,
      text: 'A **durable reply**.\n\n```rust\nfn main() {}\n```',
    },
    {
      t: 'turn_end',
      ts: 7,
      turn: 1,
      slot: 'synthetic@example.test',
      status: 'SUCCESS',
      secs: 1,
      usage: { input_tokens: 1, output_tokens: 2, thinking_tokens: 3, total_tokens: 6, context: 0 },
    },
    { t: 'notice', ts: 8, kind: 'info', text: 'Finished.', data: null },
  ];
  const { text } = sessionMarkdown(session, events);
  expect(text).toContain(`# ${session.title}\n\n${session.cwd} · ${session.model}`);
  expect(text).toContain('## you\n\nWhat does this code do?');
  expect(text).toContain(`*turn 1 · synthetic@example.test · ${session.model}*`);
  expect(text).toContain(
    '<details><summary>Thought</summary>\n\nRead the source first.\n\n</details>',
  );
  expect(text).toContain('- `run_command` cargo test (error)');
  expect(text).toContain('- `view_file` src/main.rs');
  expect(text).toContain('A **durable reply**.\n\n```rust\nfn main() {}\n```');
  expect(text).toContain('*turn 1 ended · SUCCESS*');
  expect(text).toContain('> **info**: Finished.');
});

it('retains generated prompt roles and repeated durable messages without duplicating step updates', () => {
  const events: EventRecord[] = [
    user('repeat'),
    user('repeat'),
    user('objective', 'goal'),
    user('try again', 'nudge'),
    user('scheduled command', 'job'),
    user('repeat command', 'loop'),
    user('fallback role', 'unknown'),
    { ...tool({ CommandLine: 'npm test' }), state: 'running' },
    { ...tool({ CommandLine: 'npm test' }), ts: 3 },
    { t: 'delta', ts: 4, turn: 1, step: 2, text: 'partial reply' },
    { t: 'assistant', ts: 5, turn: 1, step: 2, text: 'complete reply' },
  ];
  const { text } = sessionMarkdown(fixtureSession(), events);
  expect(text.match(/## you\n\nrepeat/g)).toHaveLength(2);
  expect(text).toContain('## goal\n\nobjective');
  expect(text).toContain('## goal push\n\ntry again');
  expect(text).toContain('## job\n\nscheduled command');
  expect(text).toContain('## loop\n\nrepeat command');
  expect(text).toContain('## you\n\nfallback role');
  expect(text.match(/`run_command`/g)).toHaveLength(1);
  expect(text).toContain('complete reply');
  expect(text).not.toContain('partial reply');
});

it('handles non-object tool parameters and unknown tool names without corrupting the export', () => {
  for (const params of [null, [], 'unexpected', 12, { CommandLine: { nested: true } }])
    expect(sessionMarkdown(fixtureSession(), [tool(params)]).text).toContain('- `run_command`');
  const { text } = sessionMarkdown(fixtureSession(), [
    { ...tool({ ServerName: 'synthetic', ToolName: 'lookup' }), name: 'call_mcp_tool' },
    { ...tool({ first: 'inspect this' }), step: 2, name: 'unknown_tool' },
    { ...tool({ first: 'prototype-named tool' }), step: 3, name: 'toString' },
    user('prototype-named role', 'constructor'),
  ]);
  expect(text).toContain('- `call_mcp_tool` synthetic · lookup');
  expect(text).toContain('- `unknown_tool` inspect this');
  expect(text).toContain('- `toString` prototype-named tool');
  expect(text).toContain('## you\n\nprototype-named role');
});

it('sanitizes and limits filenames independently of the complete title in the Markdown', () => {
  const session = { ...fixtureSession(), title: '../unsafe / name:*?\\' + 'x'.repeat(100) };
  const { filename, text } = sessionMarkdown(session, []);
  expect(filename).toMatch(/^[\w.-]{60}\.md$/);
  expect(filename).not.toMatch(/[\\/:*?]/);
  expect(text).toContain(`# ${session.title}`);
  expect(sessionMarkdown({ ...session, title: '' }, []).filename).toBe('session.md');
  expect(sessionMarkdown({ ...session, title: '' }, []).text).toContain('# Untitled');
});

function browserDownload() {
  vi.useFakeTimers();
  const anchor = { href: '', download: '', click: vi.fn(), remove: vi.fn() };
  const append = vi.fn();
  vi.stubGlobal('document', { createElement: vi.fn(() => anchor), body: { append } });
  const create = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:synthetic');
  const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
  return { anchor, append, create, revoke };
}

it('downloads a Markdown Blob and releases the temporary anchor and object URL', async () => {
  const { anchor, append, create, revoke } = browserDownload();
  const session = fixtureSession();
  downloadSession(session, [user('export me')]);
  expect(anchor.href).toBe('blob:synthetic');
  expect(anchor.download).toBe('Fixture-session.md');
  expect(append).toHaveBeenCalledWith(anchor);
  expect(anchor.click).toHaveBeenCalledOnce();
  expect(anchor.remove).toHaveBeenCalledOnce();
  const blob = create.mock.calls[0]?.[0] as Blob;
  expect(blob).toBeInstanceOf(Blob);
  expect(blob?.type).toBe('text/markdown;charset=utf-8');
  expect(await blob?.text()).toContain('export me');
  expect(revoke).not.toHaveBeenCalled();
  await vi.runAllTimersAsync();
  expect(revoke).toHaveBeenCalledExactlyOnceWith('blob:synthetic');
});

it('releases download resources when clicking the anchor fails', async () => {
  const { anchor, revoke } = browserDownload();
  anchor.click.mockImplementation(() => {
    throw new Error('Download blocked');
  });
  expect(() => downloadSession(fixtureSession(), [])).toThrow('Download blocked');
  expect(anchor.remove).toHaveBeenCalledOnce();
  await vi.runAllTimersAsync();
  expect(revoke).toHaveBeenCalledExactlyOnceWith('blob:synthetic');
});
