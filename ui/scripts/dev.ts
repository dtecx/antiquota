/** Owns the safe development entry point: fake agent, disposable data, isolated port. */
import { spawn, spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { createSandbox } from './sandbox.ts';
import { FixtureEngine } from './engine.ts';

const sandbox = await createSandbox('antiquota-dev');
const engine = new FixtureEngine(sandbox);
await engine.start();
console.log(`Development data: ${sandbox.root}`);
console.log('Fake agent only. Browser UI: http://127.0.0.1:5173');
// Vite runs under pnpm in its own process group, so stopping ends both, not only pnpm. It
// gets no input: a background group that reads the terminal would be stopped.
const ui = spawn(process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm', ['dev'], {
  cwd: join(sandbox.repository, 'ui'),
  stdio: ['ignore', 'inherit', 'inherit'],
  shell: process.platform === 'win32',
  detached: process.platform !== 'win32',
  env: {
    ...process.env,
    ANTIQUOTA_DEV_TOKEN_FILE: join(sandbox.data, 'state', 'gui-token'),
    ANTIQUOTA_DEV_ENGINE_URL: engine.url,
  },
});
let stopping = false;
async function stop() {
  if (stopping) return;
  stopping = true;
  if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(ui.pid), '/T', '/F']);
  else if (ui.pid && ui.exitCode === null) process.kill(-ui.pid, 'SIGTERM');
  await engine.stop();
}
process.once('SIGINT', () => {
  void stop();
});
process.once('SIGTERM', () => {
  void stop();
});
ui.once('error', (error) => {
  console.error(error.message);
  void stop();
});
ui.once('exit', () => {
  void stop();
});
