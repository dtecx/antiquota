/** Owns disposable fixture directories and a fake-only engine environment. */
import { mkdtemp, mkdir, realpath, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

export async function createSandbox(label: 'antiquota-dev' | 'antiquota-ui-e2e') {
  const base = join(homedir(), 'Desktop', 'test', label);
  await mkdir(base, { recursive: true });
  const resolved = await realpath(base);
  if (!resolved.split(/[\\/]/).join('/').includes('/Desktop/test/'))
    throw new Error('The fixture root must remain inside Desktop/test.');
  const root = await mkdtemp(join(resolved, 'run-'));
  const home = join(root, 'home');
  const data = join(root, 'data');
  const project = join(home, 'project');
  const script = join(root, 'fake-script.json');
  const auth = join(
    data,
    'slots',
    'one@example.com',
    '.gemini',
    'antigravity-cli',
    'antigravity-oauth-token',
  );
  for (const path of [project, join(data, 'state'), join(data, 'sessions'), join(auth, '..')])
    await mkdir(path, { recursive: true });
  await writeFile(auth, 'synthetic-account-token', { mode: 0o600 });
  await writeFile(
    join(data, 'settings.json'),
    JSON.stringify({ keep_awake: 'off', default_cwd: project }),
    { mode: 0o600 },
  );
  await writeFile(script, JSON.stringify({ reply: 'Fixture reply.', delay_ms: 300 }), {
    mode: 0o600,
  });
  const repository = resolve(import.meta.dirname, '../..');
  const executable = (name: string) =>
    join(repository, 'target', 'debug', `${name}${process.platform === 'win32' ? '.exe' : ''}`);
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    ANTIQUOTA_HOME: home,
    ANTIQUOTA_DIR: data,
    ANTIQUOTA_NO_USAGE_LINK: '1',
    ANTIQUOTA_AGY: executable('fake-agy'),
    ANTIQUOTA_UI_DIR: join(repository, 'ui', 'dist'),
    FAKE_AGY_SCRIPT: script,
    FAKE_AGY_RECORD: join(root, 'fake-calls.jsonl'),
  };
  delete env.FAKE_AGY_SCENARIO;
  delete env.STUB_SLEEP;
  return { root, home, data, project, script, repository, executable, env };
}
export type Sandbox = Awaited<ReturnType<typeof createSandbox>>;
