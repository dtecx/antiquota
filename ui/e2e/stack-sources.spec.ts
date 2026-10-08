/** Proves source comparisons, bundle imports and private permission replacement. */
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test, expect } from './fixtures';
test('copies CLI and IDE comparisons, toggles bundles and preserves every source', async ({
  page,
  engine,
}) => {
  const root = join(engine.sandbox.home, '.gemini'),
    cli = join(root, 'config'),
    ide = join(root, 'antigravity');
  const files: Record<string, string> = {
    [join(cli, 'skills', 'shared', 'SKILL.md')]:
      '---\nname: shared\ndescription: CLI skill\n---\nCLI body.',
    [join(ide, 'skills', 'shared', 'SKILL.md')]:
      '---\nname: shared\ndescription: IDE skill\n---\nIDE body.',
    [join(cli, 'mcp_config.json')]: JSON.stringify({
      mcpServers: { shared: { command: 'printf cli' } },
    }),
    [join(ide, 'mcp_config.json')]: JSON.stringify({
      mcpServers: { shared: { command: 'printf ide' } },
    }),
    [join(cli, 'hooks.json')]: JSON.stringify({ source: { enabled: true, PostToolUse: [] } }),
    [join(cli, 'plugins', 'bundle', 'plugin.json')]: JSON.stringify({
      name: 'bundle',
      description: 'Source bundle',
    }),
    [join(root, 'antigravity-cli', 'settings.json')]: JSON.stringify({
      permissions: { allow: ['command(ls)'], deny: ['command(rm)'] },
    }),
  };
  for (const [path, text] of Object.entries(files)) {
    await mkdir(join(path, '..'), { recursive: true });
    await writeFile(path, text);
  }
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.keyboard.press('ControlOrMeta+,');
  async function section(name: string) {
    await page
      .getByRole('navigation', { name: 'Settings sections' })
      .getByRole('button', { name, exact: true })
      .click();
  }
  for (const name of ['Skills', 'MCP servers']) {
    await section(name);
    const row = page.locator('[data-name="shared"]');
    await expect(row).toContainText('only in agy CLI');
    await expect(row).toContainText('only in IDE');
    await row.getByRole('button', { name: 'Copy from agy CLI' }).click();
    await expect(row).toContainText('same as agy CLI');
    await expect(row).toContainText('different from IDE');
    await page.getByRole('button', { name: 'Copy all differing from IDE' }).click();
    await expect(row).toContainText('same as IDE');
    await expect(row).toContainText('different from agy CLI');
    await row.getByRole('button', { name: 'Edit', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Edit shared' })).toHaveValue(/ide|IDE/);
    await page.getByRole('button', { name: 'Cancel editor' }).click();
  }
  await section('Hooks');
  const hook = page.locator('[data-name="source"]');
  await hook.getByRole('button', { name: 'Copy from agy CLI' }).click();
  await expect(hook.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  await hook.getByRole('switch').click();
  await expect(hook.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  await section('Plugins');
  const plugin = page.locator('[data-name="bundle"]');
  await expect(plugin).toContainText('only in agy CLI');
  await plugin.getByRole('button', { name: 'Copy from agy CLI' }).click();
  await expect(plugin).toContainText('same as agy CLI');
  await plugin.getByRole('switch').click();
  await expect(plugin.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  await plugin.getByRole('button', { name: 'Delete', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Delete plugin "bundle" from antiquota?' })
    .getByRole('button', { name: 'Delete', exact: true })
    .click();
  await expect(plugin).toContainText('only in agy CLI');
  await section('Permissions');
  await page.getByRole('button', { name: 'Copy from agy CLI', exact: true }).click();
  await expect(page.getByRole('list', { name: 'Allow without asking' })).toContainText(
    'command(ls)',
  );
  await expect(page.getByRole('list', { name: 'Always refuse' })).toContainText('command(rm)');
  await page.getByRole('button', { name: 'Remove command(rm)' }).click();
  await expect(page.getByRole('list', { name: 'Always refuse' })).not.toContainText('command(rm)');
  for (const [path, text] of Object.entries(files)) expect(await readFile(path, 'utf8')).toBe(text);
});
