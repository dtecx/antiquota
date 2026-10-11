/** Proves transcript rendering and its actions through isolated HTTP, durable history and SSE. */
import { appendFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import type { FixtureEngine } from '../scripts/engine';
import type { EventRecord } from '../src/gen/EventRecord';
import { createSession } from './account-support';
import { test, expect } from './fixtures';

const ts = 1_700_000_000;
const usage = {
  input_tokens: 1234,
  output_tokens: 567,
  thinking_tokens: 42,
  total_tokens: 1843,
  context: 0,
};
const user = (
  text = 'Inspect the transcript.',
  kind = 'message',
): Extract<EventRecord, { t: 'user' }> => ({
  t: 'user',
  ts,
  text,
  kind,
});
const reply = (text: string, step = 20): EventRecord => ({
  t: 'assistant',
  ts,
  turn: 1,
  step,
  text,
});
const tool = (
  step: number,
  name: string,
  params: Extract<EventRecord, { t: 'tool' }>['params'],
  extra: Partial<Extract<EventRecord, { t: 'tool' }>> = {},
): EventRecord => ({
  t: 'tool',
  ts,
  turn: 1,
  step,
  name,
  params,
  state: 'done',
  secs: 1.5,
  ...extra,
});
const notice = (
  kind: string,
  text: string,
  data: Extract<EventRecord, { t: 'notice' }>['data'] = {},
): EventRecord => ({ t: 'notice', ts, kind, text, data });
const start: EventRecord = {
  t: 'turn_start',
  ts,
  turn: 1,
  slot: 'one@example.com',
  model: 'gemini-3.8-flash-high',
};
const end: EventRecord = {
  t: 'turn_end',
  ts,
  turn: 1,
  slot: 'one@example.com',
  status: 'SUCCESS',
  secs: 65,
  usage,
};
const sid = (page: Page) => decodeURIComponent(new URL(page.url()).hash.slice(1));
async function history(page: Page, engine: FixtureEngine, events: EventRecord[]) {
  const id = sid(page),
    port = engine.port;
  await engine.stop();
  await appendFile(
    join(engine.sandbox.data, 'sessions', id, 'events.jsonl'),
    events.map((event) => JSON.stringify(event)).join('\n') + '\n',
  );
  await engine.start(port);
  await page.reload();
  await expect(page.getByText('Engine connected')).toBeVisible();
  return id;
}
async function shot(page: Page, engine: FixtureEngine, name: string) {
  const path = join(engine.sandbox.repository, '.handoff/evidence/ui-slice9e');
  await mkdir(path, { recursive: true });
  await page.screenshot({ path: join(path, `${name}.png`), fullPage: true });
}

test('reuses prompt text without sending, protects drafts and keeps current attachments', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Reuse prompts');
  const text = '  Earlier **prompt**.\nKeep this line.';
  await history(page, engine, [
    {
      ...user(text),
      attachments: [{ name: 'earlier.txt', path: 'synthetic/earlier.txt', image: false }],
    },
    start,
    reply('Earlier answer.'),
    end,
  ]);
  await page.locator('input[type=file]').setInputFiles({
    name: 'current.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('Current attachment'),
  });
  await expect(page.getByLabel('Attached files')).toContainText('current.txt');
  const prompt = page.getByRole('textbox', { name: 'Message', exact: true });
  await prompt.fill('Unsent draft');
  let sends = 0;
  page.on('request', (request) => {
    if (request.url().endsWith('/send')) sends++;
  });
  await page.getByRole('button', { name: 'Reuse prompt', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Replace unsent draft?' });
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(prompt).toHaveValue('Unsent draft');
  await page.getByRole('button', { name: 'Reuse prompt', exact: true }).click();
  await dialog.getByRole('button', { name: 'Replace draft', exact: true }).click();
  await expect(prompt).toHaveValue(text);
  await expect(prompt).toBeFocused();
  await expect(page.getByLabel('Attached files')).toContainText('current.txt');
  await expect(page.getByLabel('Attached files')).not.toContainText('earlier.txt');
  await expect(page.getByLabel('Your message')).toHaveCount(1);
  expect(sends).toBe(0);
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice22');
  await page.screenshot({
    path: join(directory, 'reuse-prompt.png'),
    fullPage: true,
    animations: 'disabled',
  });
});

test('finds literal conversation messages with keyboard navigation and follows latest output on request', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Conversation navigation');
  const events: EventRecord[] = [{ ...start, turn: 500 }];
  for (let i = 0; i < 20; i++)
    events.push(
      { ...user(i === 0 ? 'Needle [x] in a prompt' : `Prompt ${i}`), ts: ts + i },
      {
        t: 'assistant',
        ts: ts + i,
        turn: 500,
        step: i + 1,
        text: `${i === 12 || i === 19 ? 'needle [x]' : 'Answer'} ${i}\n\n${'Useful transcript detail.\n'.repeat(5)}${i === 19 ? '<script>literal</script>' : ''}`,
      },
    );
  events.push(
    { ...end, turn: 500 },
    { t: 'thinking', ts, turn: 500, step: 30, text: 'Private-only needle [x]', tokens: 2 },
  );
  const id = await history(page, engine, events);
  await page
    .getByRole('textbox', { name: 'Message', exact: true })
    .dispatchEvent('keydown', { key: 'f', ctrlKey: true, isComposing: true });
  await expect(page.getByRole('search', { name: 'Find in conversation', exact: true })).toHaveCount(
    0,
  );
  await page.keyboard.press('Control+f');
  const find = page.getByRole('search', { name: 'Find in conversation', exact: true });
  const input = find.getByRole('searchbox', { name: 'Find text in conversation' });
  await expect(input).toBeFocused();
  await input.fill('NEEDLE [x]');
  await expect(find).toContainText('1 of 3 messages');
  await expect(page.locator('.find-match')).toContainText('Needle [x] in a prompt');
  await input.press('Enter');
  await expect(find).toContainText('2 of 3 messages');
  await expect(page.locator('.find-match')).toContainText('needle [x] 12');
  await input.press('Shift+Enter');
  await expect(find).toContainText('1 of 3 messages');
  await find.getByRole('button', { name: 'Previous matching message' }).click();
  await expect(find).toContainText('3 of 3 messages');
  await find.getByRole('button', { name: 'Next matching message' }).click();
  await expect(find).toContainText('1 of 3 messages');
  await input.fill('private-only');
  await expect(find).toContainText('No matching messages');
  await expect(find.getByRole('button', { name: 'Next matching message' })).toBeDisabled();
  await input.fill('<script>');
  await expect(find).toContainText('1 of 1 messages');
  await expect(page.locator('.reply script')).toHaveCount(0);
  await input.fill('needle [x]');
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice22');
  for (const colorScheme of ['dark', 'light'] as const) {
    await page.emulateMedia({ colorScheme });
    await page.screenshot({
      path: join(directory, `find-${colorScheme}.png`),
      fullPage: true,
      animations: 'disabled',
    });
  }
  await page.setViewportSize({ width: 440, height: 820 });
  await page.screenshot({
    path: join(directory, 'find-narrow.png'),
    fullPage: true,
    animations: 'disabled',
  });
  expect(await find.evaluate((node) => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
  await input.press('Enter');
  const scroller = page.getByLabel('Conversation', { exact: true });
  await expect.poll(() => scroller.evaluate((node) => node.scrollTop)).toBeGreaterThan(100);
  await input.press('Escape');
  await expect(find).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: 'Find in conversation', exact: true }),
  ).toBeFocused();
  await expect.poll(() => scroller.evaluate((node) => node.scrollTop)).toBeGreaterThan(100);
  await scroller.evaluate((node) => {
    node.scrollTop = 0;
  });
  const jump = page.getByRole('button', { name: 'Jump to latest', exact: true });
  await expect(jump).toBeVisible();
  const before = await scroller.evaluate((node) => node.scrollTop);
  expect(
    (await engine.call('POST', `/api/sessions/${id}/send`, { text: 'New output while reading.' }))
      .status,
  ).toBe(200);
  await expect(page.getByLabel('Agent reply').last()).toContainText('Fixture reply.');
  expect(await scroller.evaluate((node) => node.scrollTop)).toBe(before);
  await jump.click();
  await expect(jump).toHaveCount(0);
  await expect(scroller).toBeFocused();
  await expect
    .poll(() => scroller.evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight))
    .toBeLessThanOrEqual(1);
  expect(
    (await engine.call('POST', `/api/sessions/${id}/send`, { text: 'Keep following.' })).status,
  ).toBe(200);
  await expect(page.getByLabel('Agent reply')).toHaveCount(22);
  await expect
    .poll(() => scroller.evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight))
    .toBeLessThanOrEqual(1);
  await page.screenshot({
    path: join(directory, 'latest-output.png'),
    fullPage: true,
    animations: 'disabled',
  });
});

test('find and prompt reuse target their own split pane and clear search on session swap', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Left navigation');
  await history(page, engine, [user('Left prompt'), reply('Left answer')]);
  const created = await engine.call('POST', '/api/sessions', {
    cwd: engine.sandbox.project,
    title: 'Right navigation',
  });
  const other = JSON.parse(created.text) as { id: string };
  await engine.call('POST', `/api/sessions/${other.id}/send`, { text: 'Right prompt' });
  await expect
    .poll(
      async () =>
        JSON.parse((await engine.call('GET', `/api/sessions/${other.id}`)).text).meta.status,
    )
    .toBe('idle');
  await page.reload();
  await expect(page.getByText('Engine connected')).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Session actions: Right navigation', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Split view', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Right navigation', exact: true }).click();
  const left = page.getByRole('region', { name: 'Primary session', exact: true });
  const right = page.getByRole('region', { name: 'Right session', exact: true });
  await expect(right.getByLabel('Agent reply')).toContainText('Fixture reply.');
  const leftPrompt = left.getByRole('textbox', { name: 'Message', exact: true });
  const rightPrompt = right.getByRole('textbox', { name: 'Message', exact: true });
  await leftPrompt.fill('Keep left draft');
  await rightPrompt.focus();
  await page.keyboard.press('Control+f');
  await right.getByRole('searchbox').fill('Fixture');
  await expect(right.getByRole('search')).toContainText('1 of 1 messages');
  await expect(left.getByRole('search')).toHaveCount(0);
  await leftPrompt.focus();
  await page.keyboard.press('Control+f');
  await left.getByRole('searchbox').fill('Left');
  await expect(left.getByRole('search')).toContainText('1 of 2 messages');
  await expect(right.getByRole('searchbox')).toHaveValue('Fixture');
  await right.getByRole('button', { name: 'Reuse prompt' }).click();
  await expect(rightPrompt).toHaveValue('Right prompt');
  await expect(rightPrompt).toBeFocused();
  await expect(leftPrompt).toHaveValue('Keep left draft');
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice22');
  await page.screenshot({
    path: join(directory, 'split-find.png'),
    fullPage: true,
    animations: 'disabled',
  });
  await right.getByRole('button', { name: 'Swap sides' }).click();
  await expect(page.getByRole('search')).toHaveCount(0);
  await expect(leftPrompt).toHaveValue('Right prompt');
  await expect(rightPrompt).toHaveValue('Keep left draft');
});

test('highlights safe code, wraps long lines and copies only conversation messages', async ({
  page,
  engine,
}) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await createSession(page, engine, 'Harness tools');
  const prompt = '  Explain **this code**.\nKeep the formatting.';
  const code = `// Preserve the original text\nconst count: number = 42;\nconst label = "<script> & ${'long value '.repeat(30)}";`;
  const text = `A typed example.\n\n\`\`\`typescript\n${code}\n\`\`\`\n\n\`\`\`rust\nfn main() { println!("hello"); }\n\`\`\``;
  await history(page, engine, [
    user(prompt),
    start,
    { t: 'thinking', ts, turn: 1, step: 1, text: 'Private thinking fixture', tokens: 3 },
    tool(
      2,
      'view_file',
      { AbsolutePath: '/synthetic/private-file' },
      { output: 'Private tool fixture' },
    ),
    reply(text),
    end,
  ]);
  const body = page.getByLabel('Agent reply');
  const block = body.locator('.code-block').first();
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Private unsent draft');
  const pre = block.locator('pre');
  await expect(block.locator('.code-toolbar > span')).toHaveText('typescript');
  await expect(block.locator('.hljs-keyword').first()).toHaveText('const');
  await expect(block.locator('code')).toHaveText(code);
  await expect(block.locator('script')).toHaveCount(0);
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice21');
  await mkdir(directory, { recursive: true });
  for (const colorScheme of ['dark', 'light'] as const) {
    await page.emulateMedia({ colorScheme });
    await block.evaluate((node) => node.scrollIntoView({ block: 'start' }));
    await page.screenshot({
      path: join(directory, `code-${colorScheme}.png`),
      fullPage: true,
      animations: 'disabled',
    });
    const contrast = await block.evaluate((node) => {
      const rgb = (color: string) => (color.match(/[\d.]+/g) ?? []).slice(0, 3).map(Number);
      const luminance = (color: string) =>
        rgb(color).reduce((sum, channel, index) => {
          const value = channel / 255;
          return (
            sum +
            [0.2126, 0.7152, 0.0722][index]! *
              (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4)
          );
        }, 0);
      const background = luminance(getComputedStyle(node).backgroundColor);
      return [
        '.hljs-keyword',
        '.hljs-string',
        '.hljs-number',
        '.hljs-comment',
        '.wrap-code',
        '.copy-code',
      ].map((selector) => {
        const style = getComputedStyle(node.querySelector(selector)!);
        const foreground = luminance(style.color);
        const surface = selector.endsWith('-code') ? luminance(style.backgroundColor) : background;
        return (Math.max(surface, foreground) + 0.05) / (Math.min(surface, foreground) + 0.05);
      });
    });
    expect(Math.min(...contrast)).toBeGreaterThanOrEqual(4.5);
  }
  await page.setViewportSize({ width: 440, height: 820 });
  const wrap = block.getByRole('button', { name: 'Wrap code' });
  await wrap.focus();
  await page.keyboard.press('Space');
  await expect(wrap).toHaveAttribute('aria-pressed', 'true');
  expect(await pre.evaluate((node) => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
  await block.getByRole('button', { name: 'Copy code' }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(code);
  await block.evaluate((node) => node.scrollIntoView({ block: 'start' }));
  await page.screenshot({ path: join(directory, 'code-narrow-wrapped.png'), fullPage: true });
  await page.getByRole('button', { name: 'Copy conversation', exact: true }).click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain(`## you\n\n${prompt}`);
  expect(copied).toContain(`## antiquota\n\n${text}`);
  expect(copied).not.toContain('Private thinking fixture');
  expect(copied).not.toContain('Private tool fixture');
  expect(copied).not.toContain('Private unsent draft');
  await page.screenshot({ path: join(directory, 'conversation-copy.png'), fullPage: true });
  await wrap.click();
  await expect(wrap).toHaveAttribute('aria-pressed', 'false');
});

test('renders safe Markdown, external and file links, and copies exact code and completed replies', async ({
  page,
  engine,
}) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await createSession(page, engine, 'Markdown fixture');
  await writeFile(
    join(engine.sandbox.project, 'note with spaces.md'),
    'first\nselected line\nlast\n',
  );
  const text =
    '# First\n## Second\n### Third\n#### Fourth\n\n- unordered\n+ second\n1. ordered\n2. another\n\n> quote\n> next quote\n\n| Head | Other |\n| --- | :---: |\n| value | **bold** |\n\nParagraph **strong** *emphasis* `literal <>&`\nnext line.\n\n```ts\nconst value = "<script> & **literal**";\nconsole.log(value);\n```\n\n[external](https://example.invalid/test) [unsafe](javascript:alert) [data](data:text/html) [file](file://' +
    engine.sandbox.project +
    '/note%20with%20spaces.md#L2)\n\n<script>window.compromised=true</script> <img src=x onerror="window.compromised=true">';
  await history(page, engine, [user(), start, reply(text), end]);
  const body = page.getByLabel('Agent reply');
  for (const [tag, value] of [
    ['h3', 'First'],
    ['h4', 'Second'],
    ['h5', 'Third'],
    ['h6', 'Fourth'],
  ])
    await expect(body.locator(tag!)).toHaveText(value!);
  await expect(body.locator('ul li')).toHaveText(['unordered', 'second']);
  await expect(body.locator('ol li')).toHaveText(['ordered', 'another']);
  await expect(body.locator('blockquote')).toContainText('quote');
  await expect(body.locator('table thead th')).toHaveText(['Head', 'Other']);
  await expect(body.locator('table tbody td')).toHaveText(['value', 'bold']);
  await expect(body.locator('strong').last()).toHaveText('strong');
  await expect(body.locator('em')).toHaveText('emphasis');
  await expect(body.locator('p code')).toHaveText('literal <>&');
  await expect(body.locator('script, img')).toHaveCount(0);
  await expect(body).toContainText('<script>window.compromised=true</script>');
  expect(await page.evaluate(() => Object.hasOwn(window, 'compromised'))).toBe(false);
  await expect(body.locator('a')).toHaveCount(2);
  const external = body.getByRole('link', { name: 'external' });
  await expect(external).toHaveAttribute('target', '_blank');
  await expect(external).toHaveAttribute('rel', 'noopener noreferrer');
  await page
    .context()
    .route('https://example.invalid/**', (route) =>
      route.fulfill({ body: 'Synthetic external page.' }),
    );
  const popup = page.waitForEvent('popup');
  await external.click();
  const externalPage = await popup;
  await expect(externalPage).toHaveURL('https://example.invalid/test');
  await externalPage.close();
  await body.getByRole('button', { name: 'Copy code' }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    'const value = "<script> & **literal**";\nconsole.log(value);',
  );
  await body.getByRole('button', { name: 'Copy message' }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(text);
  await shot(page, engine, 'markdown-safe');
  await body.getByRole('link', { name: 'file' }).click();
  const viewer = page.getByRole('dialog', { name: 'note with spaces.md' });
  await expect(viewer).toBeVisible();
  await expect(viewer.locator('.line.highlight .number')).toHaveText('2');
  await expect(viewer.locator('.line.highlight .text')).toHaveText('selected line');
  await shot(page, engine, 'file-link-line');
  await viewer.getByRole('button', { name: 'Close file' }).click();
});

test('identifies prompt origins, brief goals, steering, turn markers and completion usage', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Prompt origins');
  const full = 'Objective: Ship the fixture.\n\nStanding instructions stay in the tooltip.';
  await history(page, engine, [
    user(),
    start,
    reply('Done.'),
    end,
    user(full, 'goal'),
    user('Continue.', 'nudge'),
    { ...user('Steer now.'), steered: true },
    user('Job output.', 'job'),
    user('Scheduled prompt.', 'loop'),
  ]);
  await expect(page.locator('.user-message .speaker')).toHaveText([
    'YOU',
    'GOAL',
    'GOAL PUSH',
    'YOU',
    'JOB',
    'LOOP',
  ]);
  const goal = page.locator('.user-message').filter({ hasText: 'Ship the fixture.' });
  await expect(goal.locator('.message')).toHaveText('Ship the fixture.');
  await expect(goal).toHaveAttribute('title', full);
  await expect(page.locator('.steered')).toContainText('sent while agy worked');
  const marker = page.locator('.routing');
  await expect(marker).toContainText('Turn 1');
  await expect(marker).toContainText('one@example.com');
  await expect(marker).toContainText('gemini-3.8-flash-high');
  await expect(marker.locator('time')).toHaveAttribute(
    'datetime',
    new Date(ts * 1000).toISOString(),
  );
  await marker.getByRole('button', { name: 'Rewind before turn 1' }).click();
  const confirm = page.getByRole('dialog', { name: 'Rewind before turn 1?' });
  await expect(confirm).toBeVisible();
  await confirm.getByRole('button', { name: 'Cancel', exact: true }).click();
  const completion = page.locator('.turn-end');
  await expect(completion).toContainText('SUCCESS');
  await expect(completion).toContainText('1m 5s');
  await expect(completion).toContainText('1.2k in / 567 out');
  await expect(completion).toContainText('one@example.com');
  await expect(page.locator('.transcript .live')).toHaveCount(0);
  await shot(page, engine, 'prompt-origins-turns');
});

test('groups adjacent tools with unique paths, readable names, previews, full errors and chronological thinking', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Tool history');
  const path = join(engine.sandbox.project, 'notes.txt');
  const diff =
    '--- a/notes.txt\n+++ b/notes.txt\n@@ -1,4 +1,4 @@\n-old1\n+new1\n-old2\n+new2\n-old3\n+new3\n-old4\n+new4';
  const command = 'first\nsecond\nthird\nfourth\nfifth\n\u001b[31msixth\u001b[0m\n';
  const failure = 'extended failure\nsecond line\nthird line';
  await history(page, engine, [
    user(),
    start,
    tool(1, 'view_file', { AbsolutePath: path }),
    tool(2, 'view_file', { AbsolutePath: path }),
    tool(3, 'write_to_file', { TargetFile: path }, { diff }),
    tool(4, 'replace_file_content', { TargetFile: path }),
    tool(5, 'run_command', { CommandLine: 'printf fixture' }, { output: command }),
    tool(6, 'list_dir', { DirectoryPath: engine.sandbox.project }),
    tool(7, 'grep_search', { Query: 'needle' }),
    tool(8, 'find_by_name', { Pattern: '*.md' }),
    tool(9, 'read_url_content', { Url: 'https://example.invalid' }),
    tool(10, 'call_mcp_tool', { ServerName: 'fixture', ToolName: 'lookup' }),
    tool(
      11,
      'mystery_tool',
      { value: 'short argument', other: 123 },
      { state: 'error', error: failure },
    ),
    { t: 'thinking', ts, turn: 1, step: 12, text: 'A **reasoned** next step.', tokens: 42 },
    reply('Middle reply.'),
    tool(21, 'run_command', { CommandLine: 'pwd' }, { output: 'fixture folder' }),
    { t: 'thinking', ts, turn: 1, step: 22, text: 'long reasoning '.repeat(100), tokens: 0 },
    end,
  ]);
  const groups = page.locator('details.activity');
  await expect(groups).toHaveCount(2);
  const summary = groups.first().locator(':scope > summary');
  for (const value of ['Edited 1 file', 'read 1 file', '+4', '−4', '1 failed', '11 steps'])
    await expect(summary).toContainText(value);
  await summary.click();
  const rows = groups.first().locator('.tool');
  await expect(rows.locator('.name')).toHaveText([
    'Read',
    'Read',
    'Write',
    'Edit',
    'Run',
    'List',
    'Search',
    'Find',
    'Fetch',
    'MCP',
    'mystery tool',
  ]);
  await expect(rows.nth(10).locator('.arg')).toHaveText('short argument');
  await expect(rows.nth(9).locator('.arg')).toHaveText('fixture · lookup');
  await expect(rows.nth(0).locator('.state')).toHaveText('done');
  await expect(rows.nth(0).locator('.time')).toHaveText('1.5s');
  await expect(rows.nth(10).locator('.state')).toHaveText('error');
  const commandRow = rows.nth(4),
    diffRow = rows.nth(2);
  await expect(commandRow.locator('.preview')).toContainText('third\nfourth\nfifth\nsixth');
  await expect(commandRow.locator('.more')).toHaveText('… 2 more lines · click to expand');
  await expect(commandRow.locator('.preview')).not.toContainText('first');
  await commandRow.locator('.preview').click();
  await expect(commandRow).toHaveAttribute('open', '');
  await expect(commandRow.locator('.body pre')).toHaveText(
    'first\nsecond\nthird\nfourth\nfifth\nsixth\n',
  );
  await commandRow.locator(':scope > summary').click();
  await expect(commandRow).not.toHaveAttribute('open', '');
  await expect(diffRow.locator('.preview')).toContainText('-old3\n+new3');
  await expect(diffRow.locator('.more')).toHaveText('… 2 more lines · click to expand');
  await diffRow.locator('.preview').click();
  await expect(diffRow.locator('.body pre')).toHaveText(diff);
  await rows.nth(0).locator(':scope > summary').click();
  await expect(rows.nth(0).locator('.body pre')).toContainText('AbsolutePath');
  await rows.nth(10).locator(':scope > summary').click();
  await expect(rows.nth(10).locator('.body .error')).toHaveText(failure);
  await expect(page.locator('.thinking').first()).toHaveAttribute('open', '');
  await expect(page.locator('.thinking').first().locator('strong')).toHaveText('reasoned');
  await expect(page.locator('.thinking').first().locator('summary')).toContainText('42 tokens');
  await expect(page.locator('.thinking').last()).not.toHaveAttribute('open', '');
  await expect(page.locator('.thinking').last().locator('summary')).not.toContainText('tokens');
  expect(
    await page
      .locator('.turn')
      .evaluate((node) => [...node.children].map((child) => child.className.split(' ')[0])),
  ).toEqual([
    'user-message',
    'routing',
    'activity',
    'thinking',
    'reply',
    'activity',
    'thinking',
    'turn-end',
  ]);
  await shot(page, engine, 'tools-expanded');
  await page.getByRole('button', { name: 'Detailed', exact: true }).click();
  await expect(page.locator('.tool .preview')).toHaveCount(0);
  await expect(page.locator('.thinking[open]')).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole('button', { name: 'Compact', exact: true })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  await expect(page.locator('.thinking[open]')).toHaveCount(0);
  await page.getByRole('button', { name: 'Expand all', exact: true }).click();
  await expect(page.locator('details.activity[open]')).toHaveCount(2);
  await expect(page.locator('details.tool[open]')).toHaveCount(12);
  await expect(page.locator('details.thinking[open]')).toHaveCount(2);
  await page.getByRole('button', { name: 'Collapse all', exact: true }).click();
  await expect(
    page.locator('details.activity[open], details.tool[open], details.thinking[open]'),
  ).toHaveCount(0);
  await page.getByRole('button', { name: 'Compact', exact: true }).click();
  await expect(page.locator('.thinking[open]')).toHaveCount(1);
  await shot(page, engine, 'tools-detailed-restored');
  await page.reload();
  await expect(page.getByRole('button', { name: 'Detailed', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.locator('.thinking[open]')).toHaveCount(1);
});

test('keeps expanded running tool nodes during real SSE completion and clears failed live replies', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Live tool fixture');
  const step = {
    conversation_id: 'live-fixture',
    step_index: 1,
    step_type: 'tool',
    tool_name: 'run_command',
    tool_info: { name: 'run_command', parameters: { CommandLine: 'printf live' } },
  };
  await engine.scenario({
    write_notes: false,
    actions: [
      { event: { event: 'init', conversation_id: 'live-fixture' } },
      { event: { event: 'step_update', step_update: { ...step, state: 'ACTIVE' } } },
      {
        delay_ms: 2500,
        event: {
          event: 'step_update',
          step_update: {
            ...step,
            state: 'DONE',
            duration_seconds: 2.5,
            tool_info: { ...step.tool_info, output: 'complete output' },
          },
        },
      },
      {
        event: {
          event: 'step_update',
          step_update: {
            conversation_id: 'live-fixture',
            step_index: 2,
            step_type: 'agent_response',
            state: 'ACTIVE',
            text_delta: 'Partial preserved reply.',
          },
        },
      },
      {
        delay_ms: 1000,
        event: {
          event: 'result',
          result: { conversation_id: 'live-fixture', status: 'ERROR', error: 'synthetic failure' },
        },
      },
    ],
  });
  const prompt = page.getByRole('textbox', { name: 'Message', exact: true });
  await prompt.fill('Run the live fixture.');
  await prompt.press('Enter');
  const group = page.locator('details.activity'),
    row = group.locator('.tool');
  await expect(group).toHaveClass(/running/);
  await expect(group.locator(':scope > summary')).toContainText('Run printf live…');
  await group.locator(':scope > summary').click();
  await expect(row.locator('.state')).toHaveText('running');
  await row.locator(':scope > summary').click();
  await row.evaluate((node) => node.setAttribute('data-identity', 'preserved'));
  await expect(row.locator('.state')).toHaveText('done');
  await expect(row).toHaveAttribute('data-identity', 'preserved');
  await expect(row).toHaveAttribute('open', '');
  await expect(row.locator('.time')).toHaveText('2.5s');
  await expect(row.locator('.body')).toContainText('complete output');
  await expect(page.locator('.session-state .status')).toHaveText('idle');
  await expect(page.locator('.turn-end')).toContainText('ERROR');
  await expect(page.getByRole('alert').last()).toContainText('synthetic failure');
  await expect(page.locator('.transcript .live, .activity.running')).toHaveCount(0);
  await shot(page, engine, 'live-tool-ended-failure');
});

test('presents typed notices, partial verifier data and inert errors without interpreting malformed fields', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Notice fixture');
  await history(page, engine, [
    user(),
    notice('handoff', 'Moving one@example.com to two@example.com.'),
    notice('wall', 'Quota resets later.'),
    notice('noaccount', 'Sign in to continue.'),
    notice('goal', 'Met objective.', { met: true }),
    notice('goal', 'More work needed.', { met: false }),
    notice('check', 'Successful check.', { passed: true, output: 'check output\nlast line' }),
    notice('check', 'Failed check.', { passed: false, output: 'failure output' }),
    notice('verdict', 'Verified complete.', {
      done: true,
      missing: ['first missing item', 'second missing item'],
      slot: 'verifier@example.com',
      model: 'gemini-3.8-flash-high',
    }),
    notice('verdict', 'Not yet met.', { done: false }),
    notice('verdict', 'Uncertain.', { done: null, slot: 'partial@example.com' }),
    notice('verdict', 'Malformed verifier.', {
      done: 'false',
      missing: [false, null, 7],
      model: false,
    }),
    notice('error', '<img src=x onerror="window.compromised=true">\n**literal error**'),
    notice('rewind', 'Historical rewind.', {
      files: ['inside.txt', 'outside.txt'],
      outside: ['outside.txt', 'outside-only.txt'],
    }),
    notice('unknown', 'Null data survives.', null),
    notice('unknown', 'False data survives.', false),
  ]);
  for (const label of [
    'Account handoff',
    'Out of quota',
    'No account',
    'Goal met',
    'Goal not met',
    'Check passed',
    'Check failed',
    'Verifier · done',
    'Verifier · not met',
    'Error',
    'Rewind',
  ])
    await expect(
      page
        .locator('.notice')
        .filter({ has: page.locator('strong', { hasText: label }) })
        .first(),
    ).toBeVisible();
  const check = page.getByLabel('Check passed');
  await check.getByText('Output', { exact: true }).click();
  await expect(check.locator('pre')).toHaveText('check output\nlast line');
  const verdict = page.getByLabel('Verifier · done');
  await expect(verdict.locator('li')).toHaveText(['first missing item', 'second missing item']);
  await expect(verdict).toContainText('verifier@example.com · gemini-3.8-flash-high');
  await expect(
    page.getByLabel('Verifier · indeterminate').filter({ hasText: 'Uncertain.' }),
  ).toContainText('partial@example.com');
  await expect(
    page
      .getByLabel('Verifier · indeterminate')
      .filter({ hasText: 'Malformed verifier.' })
      .locator('li,small'),
  ).toHaveCount(0);
  await expect(page.getByRole('alert').locator('pre')).toHaveText(
    '<img src=x onerror="window.compromised=true">\n**literal error**',
  );
  await expect(page.getByRole('alert').locator('img,strong strong')).toHaveCount(0);
  const rewind = page.getByLabel('Rewind', { exact: true });
  await expect(rewind.locator('li')).toHaveText([
    'inside.txt',
    "outside.txt (it had changed after the agent's last edit)",
    "outside-only.txt (it had changed after the agent's last edit)",
  ]);
  expect((await page.locator('.notice').allTextContents()).join(' ')).not.toContain('undefined');
  await shot(page, engine, 'typed-notices');
  await page
    .getByLabel('No account', { exact: true })
    .getByRole('button', { name: 'Add Google account' })
    .click();
  await expect(page.getByRole('dialog', { name: 'Add Google account' })).toBeVisible();
  await shot(page, engine, 'notice-sign-in');
});

test('adds vetted denied command rules, retains deny rules, and sends retries only after granting permissions', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Denied commands');
  expect(
    (
      await engine.call('PUT', '/api/permissions', {
        allow: ['command(pwd)'],
        deny: ['command(git status)'],
      })
    ).status,
  ).toBe(200);
  const id = await history(page, engine, [
    user(),
    notice('denied', 'Denied command.', {
      commands: ['git status', 'git status'],
      rules: ['command(*)'],
    }),
    notice('denied', 'Runner remains per-call.', {
      commands: ['/bin/bash -c echo', 'find . -exec sh {} ;'],
      rules: ['command(*)'],
    }),
  ]);
  const safe = page.getByLabel('Permission denied').filter({ hasText: 'Denied command.' });
  await expect(
    safe.getByRole('button', { name: 'Allow command(git status)', exact: true }),
  ).toBeVisible();
  await expect(
    page
      .getByLabel('Permission denied')
      .filter({ hasText: 'Runner remains per-call.' })
      .getByRole('button'),
  ).toHaveCount(0);
  await safe.getByRole('button', { name: 'Allow command(git status)', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Allowed command(git status)');
  let permissions = JSON.parse((await engine.call('GET', '/api/settings')).text).permissions;
  expect(permissions.allow).toEqual(['command(pwd)', 'command(git status)']);
  expect(permissions.deny).toEqual(['command(git status)']);
  expect(
    JSON.parse((await engine.call('GET', `/api/sessions/${id}`)).text).events.filter(
      (event: EventRecord) => event.t === 'user',
    ),
  ).toHaveLength(1);
  await engine.scenario({ reply: 'Retried fixture.' });
  await safe.getByRole('button', { name: 'Allow & retry', exact: true }).click();
  await expect(page.getByLabel('Your message').last()).toContainText(
    'Permission granted (command(git status)). Retry what was denied and continue.',
  );
  await expect(page.getByLabel('Agent reply')).toContainText('Retried fixture.');
  permissions = JSON.parse((await engine.call('GET', '/api/settings')).text).permissions;
  expect(permissions.allow).toEqual(['command(pwd)', 'command(git status)']);
  expect(permissions.deny).toEqual(['command(git status)']);
  await shot(page, engine, 'denied-command-retry');
});

test('retries denied edits in accept-edits mode and refuses to rewind over later user edits', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Denied edit fixture');
  const id = await history(page, engine, [
    user(),
    notice('denied', 'Denied write.', {
      commands: [],
      actions: [{ action: 'write_to_file', display_name: 'Write notes' }],
    }),
  ]);
  await page.getByRole('button', { name: 'Switch to accept edits & retry' }).click();
  await expect(page.getByLabel('Agent reply')).toBeVisible();
  const session = JSON.parse((await engine.call('GET', `/api/sessions/${id}`)).text);
  expect(session.meta.mode).toBe('accept-edits');
  await expect(page.getByLabel('Your message').last()).toContainText(
    'Edits are now allowed. Retry what was denied and continue.',
  );
  await writeFile(join(engine.sandbox.project, 'notes.txt'), 'later user bytes\n');
  await page.getByRole('button', { name: 'Rewind before turn 1' }).click();
  await page
    .getByRole('dialog', { name: 'Rewind before turn 1?' })
    .getByRole('button', { name: 'Rewind', exact: true })
    .click();
  await expect(page.getByRole('alert').last()).toContainText('notes.txt');
  expect(await readFile(join(engine.sandbox.project, 'notes.txt'), 'utf8')).toBe(
    'later user bytes\n',
  );
  await shot(page, engine, 'rewind-preserves-user-edit');
});

test('follows streamed events at 120 pixels and preserves scroll beyond that boundary', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Scroll boundary');
  const message = (text_delta: string) => ({
    event: 'step_update',
    step_update: {
      conversation_id: 'scroll-fixture',
      step_index: 1,
      step_type: 'agent_response',
      state: 'ACTIVE',
      text_delta,
    },
  });
  await engine.scenario({
    write_notes: false,
    actions: [
      { event: { event: 'init', conversation_id: 'scroll-fixture' } },
      { event: message('line\n'.repeat(200)) },
      { delay_ms: 3000, event: message('\nOUTSIDE BOUNDARY\n'.repeat(10)) },
      { delay_ms: 3000, event: message('\nAT BOUNDARY\n'.repeat(10)) },
      {
        delay_ms: 500,
        event: {
          event: 'result',
          result: { conversation_id: 'scroll-fixture', status: 'SUCCESS' },
        },
      },
    ],
  });
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Long stream.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByLabel('Agent reply')).toContainText('line');
  const scroller = page.getByLabel('Conversation', { exact: true });
  await expect
    .poll(() => scroller.evaluate((node) => node.scrollHeight - node.clientHeight))
    .toBeGreaterThan(1000);
  const before = await scroller.evaluate((node) => {
    node.scrollTop = node.scrollHeight - node.clientHeight - 121;
    node.dispatchEvent(new Event('scroll'));
    return node.scrollTop;
  });
  await expect(page.getByLabel('Agent reply')).toContainText('OUTSIDE BOUNDARY');
  expect(await scroller.evaluate((node) => node.scrollTop)).toBe(before);
  await scroller.evaluate((node) => {
    node.scrollTop = node.scrollHeight - node.clientHeight - 120;
    node.dispatchEvent(new Event('scroll'));
  });
  await expect(page.getByLabel('Agent reply')).toContainText('AT BOUNDARY');
  await expect
    .poll(() => scroller.evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight))
    .toBeLessThanOrEqual(1);
  await expect(page.locator('.session-state .status')).toHaveText('idle');
  await expect(page.locator('.transcript .live')).toHaveCount(0);
  await shot(page, engine, 'scroll-boundary-follow');
});

test('a delayed drawer file cannot replace a newer Markdown viewer', async ({ page, engine }) => {
  await createSession(page, engine, 'Viewer ownership');
  await writeFile(join(engine.sandbox.project, 'old.txt'), 'old drawer contents\n');
  await writeFile(join(engine.sandbox.project, 'new.txt'), 'new Markdown contents\n');
  await history(page, engine, [
    user(),
    reply('[New file](file://' + engine.sandbox.project + '/new.txt)'),
  ]);
  let release: () => void = () => {};
  const blocked = new Promise<void>((resolve) => {
    release = resolve;
  });
  let finished: () => void = () => {};
  const completed = new Promise<void>((resolve) => {
    finished = resolve;
  });
  let requested = false;
  await page.route('**/api/sessions/*/file?*', async (route) => {
    if (new URL(route.request().url()).searchParams.get('path') !== 'old.txt')
      return route.continue();
    const response = await route.fetch();
    requested = true;
    await blocked;
    await route.fulfill({ response });
    finished();
  });
  try {
    await page.getByRole('button', { name: 'Changes and files' }).click();
    await page.getByRole('tab', { name: 'Files', exact: true }).click();
    await page
      .getByRole('list', { name: 'Folder contents' })
      .locator('.row', { hasText: 'old.txt' })
      .click();
    await expect.poll(() => requested).toBe(true);
    await page.getByLabel('Agent reply').getByRole('link', { name: 'New file' }).click();
    const viewer = page.getByRole('dialog', { name: 'new.txt', exact: true });
    await expect(viewer.getByLabel('File contents')).toContainText('new Markdown contents');
    release();
    await completed;
    await expect(viewer).toBeVisible();
    await expect(page.getByRole('dialog', { name: 'old.txt', exact: true })).toHaveCount(0);
    await shot(page, engine, 'viewer-ownership');
  } finally {
    release();
  }
});
