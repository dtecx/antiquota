/** Proves settings and the direct Chart.js view against the isolated Rust engine. */
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { test, expect } from './fixtures';
import type { FixtureEngine } from '../scripts/engine.ts';
import type { SettingsOverview } from '../src/gen/SettingsOverview';
async function section(page: Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name, exact: true })
    .click();
  await expect(page.getByRole('heading', { name, exact: true }).first()).toBeVisible();
}
async function open(page: Page, engine: FixtureEngine) {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.keyboard.press('ControlOrMeta+,');
  await expect(page.getByRole('combobox', { name: 'Default model family' })).toBeVisible();
}
async function screenshot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice9b');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}
async function overview(engine: FixtureEngine): Promise<SettingsOverview> {
  return JSON.parse((await engine.call('GET', '/api/settings')).text);
}
async function accentBar(page: Page, count: number) {
  const colors = await page.locator('.usage canvas').evaluate((canvas, count) => {
    const chart = canvas as HTMLCanvasElement;
    const pixel = chart
      .getContext('2d')!
      .getImageData(
        Math.floor((chart.width * (count - 0.5)) / count),
        chart.height - 12,
        1,
        1,
      ).data;
    const sample = document.createElement('canvas');
    sample.width = sample.height = 1;
    const context = sample.getContext('2d')!;
    context.fillStyle = getComputedStyle(chart).getPropertyValue('--baton').trim();
    context.fillRect(0, 0, 1, 1);
    return { actual: [...pixel], expected: [...context.getImageData(0, 0, 1, 1).data] };
  }, count);
  expect(colors.actual).toEqual(colors.expected);
}
test('persists defaults, goals and device controls without replacing focused drafts', async ({
  page,
  engine,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const models = [
    { id: 'gemini-3.8-flash-high', label: 'Gemini Flash (High)' },
    { id: 'gemini-3.8-flash-low', label: 'Gemini Flash (Low)' },
    { id: 'gemini-pro-high', label: 'Gemini Pro (High)' },
    { id: 'gemini-pro-low', label: 'Gemini Pro (Low)' },
  ];
  await engine.scenario({ metadata: { models } });
  await writeFile(join(engine.sandbox.data, 'state', 'models.json'), JSON.stringify(models));
  await open(page, engine);
  await expect(
    page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button'),
  ).toHaveCount(13);
  for (const mode of ['accept-edits', 'plan', 'bypass', 'default']) {
    await page.getByLabel('Default mode', { exact: true }).selectOption(mode);
    await expect.poll(async () => (await overview(engine)).defaults.mode).toBe(mode);
  }
  const family = page.getByLabel('Default model family'),
    effort = page.getByLabel('Default effort');
  const families = await family.locator('option').allTextContents();
  await family.selectOption({ label: families[1]! });
  const variants = await effort
    .locator('option')
    .evaluateAll((options) => options.map((option) => (option as HTMLOptionElement).value));
  await effort.selectOption(variants.at(-1)!);
  await expect.poll(async () => (await overview(engine)).defaults.model).toBe(variants.at(-1));
  const folder = page.getByLabel('Default folder');
  await folder.fill(` ${engine.sandbox.project} `);
  await folder.press('Tab');
  await expect
    .poll(async () => (await overview(engine)).defaults.default_cwd)
    .toBe(engine.sandbox.project);
  for (const [label, key] of [
    ['Separate git worktree by default', 'worktree_default'],
    ['Keep awake while agents work', 'keep_awake'],
  ] as const) {
    await page.getByLabel(label).check();
    await expect.poll(async () => (await overview(engine)).defaults[key]).toBe('on');
    await page.getByLabel(label).uncheck();
    await expect.poll(async () => (await overview(engine)).defaults[key]).toBe('off');
  }
  for (const label of [
    'A turn finishes',
    'A goal is met',
    'agy needs your approval',
    'A background job finishes',
    'Every account is out of quota',
  ])
    await page.getByLabel(label, { exact: true }).uncheck();
  await page.getByLabel('Also while antiquota is in front').check();
  await screenshot(page, engine, 'general-dark');
  await section(page, 'Agents & goals');
  for (const [label, key, limits] of [
    ['Rounds', 'max_rounds', ['1', '500']],
    ['Approval wait', 'approval_timeout', ['1', '240']],
    ['Retries after a denial', 'auto_continue', ['0', '10']],
  ] as const) {
    for (const value of limits) {
      await page.getByRole('spinbutton', { name: label, exact: true }).fill(value);
      await page.getByRole('spinbutton', { name: label, exact: true }).press('Tab');
      await expect.poll(async () => (await overview(engine)).defaults[key]).toBe(value);
    }
  }
  await expect(page.getByText('In plan mode agy investigates', { exact: false })).toBeVisible();
  const verifier = page.getByLabel('Verifier model');
  const last = await verifier.locator('option').last().getAttribute('value');
  await verifier.selectOption(last!);
  await expect.poll(async () => (await overview(engine)).defaults.verifier_model).toBe(last);
  await section(page, 'Appearance');
  await page.getByRole('button', { name: 'Every step', exact: true }).click();
  await page.getByLabel('Previews and thinking').uncheck();
  await page.getByLabel('Times on turns').uncheck();
  await expect(page.getByText('Use your browser’s zoom controls:', { exact: false })).toBeVisible();
  expect(await page.locator('body').evaluate((body) => getComputedStyle(body).zoom)).toBe('1');
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.reload();
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.keyboard.press('ControlOrMeta+,');
  await expect(page.getByLabel('A turn finishes', { exact: true })).not.toBeChecked();
  await expect(page.getByLabel('Also while antiquota is in front')).toBeChecked();
  await section(page, 'Appearance');
  await expect(page.getByRole('button', { name: 'Every step', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.getByLabel('Previews and thinking')).not.toBeChecked();
  await expect(page.getByLabel('Times on turns')).not.toBeChecked();
  await section(page, 'Keyboard');
  for (const command of [
    '/effort',
    '/memory',
    '/permissions',
    '/delete',
    '/skills',
    '/hooks',
    '/mcp',
  ])
    await expect(page.locator('.shortcuts dt').filter({ hasText: command })).toBeVisible();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.keyboard.press('ControlOrMeta+,');
  await folder.fill('');
  await folder.press('Tab');
  await expect.poll(async () => (await overview(engine)).defaults.default_cwd).toBe('');
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.getByRole('button', { name: 'New session' }).first().click();
  await expect(page.getByLabel('Project folder')).toHaveValue(engine.sandbox.home);
  expect(errors).toEqual([]);
});
test('edits and imports only the private stack with guarded JSON and deletion', async ({
  page,
  engine,
}) => {
  const source = join(engine.sandbox.home, '.gemini', 'config', 'rules', 'source.md');
  await mkdir(join(source, '..'), { recursive: true });
  await writeFile(source, 'Source rule.');
  await open(page, engine);
  await section(page, 'Rules');
  const row = page.locator('[data-name="source.md"]');
  await expect(row).toContainText('only in agy CLI');
  await row.getByRole('button', { name: 'Copy from agy CLI' }).click();
  await expect(row).toContainText('same as agy CLI');
  await row.getByRole('button', { name: 'Edit', exact: true }).click();
  const editor = page.getByRole('textbox', { name: 'Edit source.md' });
  await expect(editor).toHaveValue('Source rule.');
  await editor.fill('Private draft.');
  await page.getByRole('button', { name: 'Save editor' }).click();
  await expect(row).toContainText('different from agy CLI');
  expect(await readFile(source, 'utf8')).toBe('Source rule.');
  await row.getByRole('button', { name: 'Edit', exact: true }).click();
  await expect(editor).toHaveValue('Private draft.');
  await editor.fill('Canceled draft.');
  await page.getByRole('button', { name: 'Cancel editor' }).click();
  await row.getByRole('button', { name: 'Edit', exact: true }).click();
  await expect(editor).toHaveValue('Private draft.');
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.keyboard.press('ControlOrMeta+,');
  await section(page, 'Rules');
  await expect(editor).toHaveCount(0);
  await page.getByRole('button', { name: 'Copy all differing from agy CLI' }).click();
  await expect(row).toContainText('same as agy CLI');
  await page.getByRole('textbox', { name: 'New rule name' }).fill('new-rule');
  await page.getByRole('button', { name: 'New rule', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Edit new-rule.md' })).toHaveValue(
    /trigger: always_on/,
  );
  await page.getByRole('button', { name: 'Cancel editor' }).click();
  await page.getByRole('button', { name: 'Edit GEMINI.md' }).click();
  await page.getByRole('textbox', { name: 'Edit GEMINI.md' }).fill('Private project instructions.');
  await page.getByRole('button', { name: 'Save editor' }).click();
  for (const [name, kind] of [
    ['Skills', 'skill'],
    ['Hooks', 'hook'],
    ['MCP servers', 'mcp'],
  ] as const) {
    await section(page, name);
    await page.getByRole('textbox', { name: `New ${kind} name` }).fill(`new-${kind}`);
    await page
      .getByRole('button', { name: kind === 'mcp' ? 'Add server' : `New ${kind}`, exact: true })
      .click();
    await expect(page.getByRole('textbox', { name: `Edit new-${kind}` })).toHaveValue(
      kind === 'skill' ? /name: new-skill/ : kind === 'hook' ? /PostToolUse/ : /"args": \[\]/,
    );
    if (kind !== 'skill') {
      await page.getByRole('textbox', { name: `Edit new-${kind}` }).fill('{invalid');
      await page.getByRole('button', { name: 'Save editor' }).click();
      await expect(page.getByRole('alert')).toBeVisible();
      await page
        .getByRole('textbox', { name: `Edit new-${kind}` })
        .fill('{"command":"printf fixture","enabled":true}');
      await page.getByRole('button', { name: 'Save editor' }).click();
      await page.getByRole('switch', { name: `Enable new-${kind}` }).click();
      await expect(page.getByRole('switch', { name: `Enable new-${kind}` })).toHaveAttribute(
        'aria-checked',
        'false',
      );
    } else {
      await expect(page.getByRole('textbox', { name: 'Edit new-skill' })).toHaveValue(
        /description:/,
      );
      await page.getByRole('button', { name: 'Save editor' }).click();
    }
  }
  await screenshot(page, engine, 'private-stack');
  await section(page, 'Rules');
  await row.getByRole('button', { name: 'Delete', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Delete rule "source.md" from antiquota?' })
    .getByRole('button', { name: 'Cancel', exact: true })
    .click();
  await expect(row).toBeVisible();
  await row.getByRole('button', { name: 'Delete', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Delete rule "source.md" from antiquota?' })
    .getByRole('button', { name: 'Delete', exact: true })
    .click();
  await expect(row).toContainText('only in agy CLI');
  expect(await readFile(source, 'utf8')).toBe('Source rule.');
  await section(page, 'Plugins');
  await expect(page.getByText('Nothing here yet.', { exact: true })).toBeVisible();
  await section(page, 'Engine');
  await expect(page.getByText('antiquota decides each tool call (hook on)')).toBeVisible();
  await writeFile(join(engine.sandbox.data, 'state', 'engine.log'), 'Settings log fixture.\n');
  await page.getByRole('button', { name: 'Engine log', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Close log' })).toBeVisible();
  await expect(page.locator('.log pre')).toHaveText('Settings log fixture.');
  await page.getByRole('button', { name: 'Close log' }).click();
  await page.getByRole('button', { name: 'Copy everything again from agy CLI' }).click();
  await page
    .getByRole('dialog', { name: 'Replace antiquota’s private configuration?' })
    .getByRole('button', { name: 'Cancel', exact: true })
    .click();
  await page.getByRole('button', { name: 'Copy everything again from agy CLI' }).click();
  await page
    .getByRole('dialog', { name: 'Replace antiquota’s private configuration?' })
    .getByRole('button', { name: 'Replace', exact: true })
    .click();
  await expect
    .poll(async () => (await overview(engine)).rules.some((rule) => rule.name === 'new-rule.md'))
    .toBe(false);
  await expect
    .poll(async () =>
      (await overview(engine)).hooks.some((hook) => hook.name === 'antiquota' && hook.enabled),
    )
    .toBe(true);
  await page.getByRole('button', { name: 'Restart engine' }).click();
  await page
    .getByRole('dialog', { name: 'Restart the engine?' })
    .getByRole('button', { name: 'Cancel', exact: true })
    .click();
  expect((await engine.call('GET', '/api/health')).status).toBe(200);
});
test('renders exact usage with bounded hover and keyboard tooltips in both themes', async ({
  page,
  engine,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  const rows = [
    {
      t: yesterday.getTime() / 1000,
      slot: 'small@example.com',
      model: 'Small',
      input: 10,
      output: 10,
    },
    {
      t: yesterday.getTime() / 1000,
      slot: 'large@example.com',
      model: 'Large',
      input: 30,
      output: 10,
    },
    { t: Date.now() / 1000, slot: 'large@example.com', model: 'Large', input: 60, output: 20 },
  ];
  await writeFile(
    join(engine.sandbox.data, 'state', 'usage.jsonl'),
    rows.map((row) => JSON.stringify(row)).join('\n') + '\n',
  );
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.getByRole('button', { name: 'Accounts and quota' }).click();
  await expect(page.getByRole('heading', { name: 'Usage', exact: true }).first()).toBeVisible();
  const usage = page.getByRole('region', { name: 'Usage', exact: true });
  await expect(usage.locator('.stats')).toContainText('140');
  await expect(usage.locator('.stats')).toContainText('3 turns');
  await expect(
    usage.getByRole('table', { name: 'Usage by account' }).locator('tbody tr').first(),
  ).toContainText('large@example.com120802');
  await expect(
    usage.getByRole('table', { name: 'Usage by model' }).locator('tbody tr').first(),
  ).toContainText('Large12086%');
  for (const days of [7, 14, 30]) {
    await usage.getByRole('button', { name: `${days} days`, exact: true }).click();
    const bars = usage.getByRole('group', { name: 'Daily usage' }).getByRole('button');
    await expect(bars).toHaveCount(days);
    for (const index of [0, Math.floor(days / 2), days - 1]) {
      await bars.nth(index).hover();
      const tip = usage.getByRole('tooltip'),
        plot = usage.locator('.plot');
      await expect(tip).toBeVisible();
      const tipBox = await tip.boundingBox(),
        plotBox = await plot.boundingBox();
      expect(tipBox!.x).toBeGreaterThanOrEqual(plotBox!.x - 1);
      expect(tipBox!.x + tipBox!.width).toBeLessThanOrEqual(plotBox!.x + plotBox!.width + 1);
    }
    await bars.nth(days - 2).focus();
    await expect(usage.getByRole('tooltip')).toContainText('60 tokens · 2 turns');
    expect(await usage.getByRole('tooltip').locator('.contribution').allTextContents()).toEqual([
      'large@example.com40',
      'small@example.com20',
    ]);
  }
  await accentBar(page, 30);
  await expect(usage.locator('.stats')).toContainText('47');
  await screenshot(page, engine, 'usage-dark');
  await page.route('**/api/usage?days=7', (route) =>
    route.fulfill({ status: 503, body: 'Usage temporarily unavailable' }),
  );
  await usage.getByRole('button', { name: '7 days', exact: true }).click();
  await expect(usage.getByRole('alert')).toBeVisible();
  await expect(usage.locator('.stats dt').first()).toHaveText('30 days');
  await expect(usage.getByRole('group', { name: 'Daily usage' }).getByRole('button')).toHaveCount(
    30,
  );
  await expect(
    usage.getByRole('table', { name: 'Usage by account' }).locator('thead'),
  ).toContainText('30 days');
  await page.unroute('**/api/usage?days=7');
  await usage.getByRole('button', { name: 'Retry usage', exact: true }).click();
  await expect(usage.locator('.stats dt').first()).toHaveText('7 days');
  await page.emulateMedia({ colorScheme: 'light' });
  await page.setViewportSize({ width: 640, height: 820 });
  await page.reload();
  await expect(page.getByText('Engine connected')).toHaveCount(1);
  await page.keyboard.press('ControlOrMeta+,');
  await section(page, 'Usage');
  const bars = usage.getByRole('group', { name: 'Daily usage' }).getByRole('button');
  await expect(bars).toHaveCount(14);
  await bars.last().hover();
  await accentBar(page, 14);
  await screenshot(page, engine, 'usage-light-narrow');
  expect(errors).toEqual([]);
  await writeFile(join(engine.sandbox.data, 'state', 'usage.jsonl'), '');
  await usage.getByRole('button', { name: '7 days', exact: true }).click();
  await expect(usage.getByText('No turns in this period yet.', { exact: true })).toBeVisible();
  await expect(usage.locator('.stats')).toContainText('0 turns');
});
