/** Proves approval cards read correctly and answers replace their pending card. */
import { describe, expect, it } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import {
  type ApprovalEvent,
  alwaysChoice,
  answerSummary,
  approvalKind,
  approvalPreview,
  approvalTarget,
  approvalTitle,
  waitMinutes,
} from './approvals';
import { ruleProblem } from './permissions';
import { projectHistory, reduceTranscript } from './transcript';
import { isBusy, statusLabel } from './format';

const base: ApprovalEvent = {
  t: 'approval',
  ts: 100,
  id: 'a1',
  session: 's1',
  tool: 'run_command',
  args: { CommandLine: 'npm test' },
  rule: 'command(npm test)',
  state: 'pending',
  why: '',
  deadline: 100 + 15 * 60,
};

describe('approval cards', () => {
  it('describe commands with their permanent rule and wait', () => {
    expect(approvalKind(base)).toBe('command');
    expect(approvalTarget(base, '/p')).toBe('npm test');
    expect(approvalTitle(base, '/p')).toBe('agy wants to run a command');
    expect(approvalPreview(base)).toBe('npm test');
    expect(waitMinutes(base)).toBe(15);
    expect(alwaysChoice(base)?.label).toBe('Always allow command(npm test)');
  });
  it('offer edits for the session only on unruled file changes', () => {
    const edit: ApprovalEvent = {
      ...base,
      tool: 'write_to_file',
      rule: '',
      args: {
        TargetFile: '/p/src/a.ts',
        CodeContent: Array.from({ length: 20 }, (_, i) => `l${i}`).join('\n'),
      },
    };
    expect(approvalTarget(edit, '/p')).toBe('src/a.ts');
    expect(approvalTitle(edit, '/p')).toBe('agy wants to edit src/a.ts');
    expect(approvalPreview(edit).split('\n')).toHaveLength(14);
    expect(alwaysChoice(edit)?.label).toBe('Allow all edits in this session');
    expect(alwaysChoice({ ...base, rule: '' })).toBeNull();
  });
  it('show MCP calls with their nested input', () => {
    const mcp: ApprovalEvent = {
      ...base,
      tool: 'call_mcp_tool',
      rule: 'mcp(olx/search)',
      args: { ServerName: 'olx', ToolName: 'search', Arguments: { filters: { city: 'Warszawa' } } },
    };
    expect(approvalKind(mcp)).toBe('mcp');
    expect(approvalTitle(mcp, '/p')).toBe('agy wants to call olx/search');
    expect(approvalPreview(mcp)).toContain('"city": "Warszawa"');
  });
  it('summarise answers', () => {
    expect(answerSummary({ ...base, state: 'allowed' })).toBe('once');
    expect(answerSummary({ ...base, state: 'allowed', always: true } as ApprovalEvent)).toBe(
      'always: command(npm test)',
    );
    expect(
      answerSummary({ ...base, rule: '', state: 'allowed', always: true } as ApprovalEvent),
    ).toBe('edits allowed for this session');
    expect(answerSummary({ ...base, state: 'denied', why: 'not now' })).toBe('not now');
    expect(answerSummary({ ...base, state: 'denied' })).toBe('by you');
  });
});

describe('approval projection', () => {
  it('replaces a pending card with its answer in place', () => {
    const notice: EventRecord = { t: 'notice', ts: 101, kind: 'info', text: 'later', data: null };
    const answered: EventRecord = { ...base, ts: 102, state: 'allowed' };
    const history = projectHistory([base, notice, answered]);
    expect(history.map((event) => event.t)).toEqual(['approval', 'notice']);
    expect(history[0]).toMatchObject({ state: 'allowed' });
    const live = reduceTranscript(reduceTranscript([base], notice), answered);
    expect(live).toEqual(history);
    const other = reduceTranscript(live, { ...base, id: 'a2', ts: 103 });
    expect(other).toHaveLength(3);
  });
});

describe('statuses and rules', () => {
  it('label waiting states and treat them as busy', () => {
    expect(statusLabel('approval')).toBe('needs your approval');
    expect(statusLabel('running')).toBe('working');
    expect(statusLabel('idle')).toBe('idle');
    expect(isBusy('approval')).toBe(true);
    expect(isBusy('idle')).toBe(false);
  });
  it('check rules before saving', () => {
    expect(ruleProblem('  ', [])).toBe('Type a rule first.');
    expect(ruleProblem('git status', [])).toContain('command(git status)');
    expect(ruleProblem('command(ls)', ['command(ls)'])).toContain('already');
    expect(ruleProblem('mcp(olx/*)', [])).toBe('');
    expect(ruleProblem(`command(${'x'.repeat(300)})`, [])).toContain('300');
  });
});
