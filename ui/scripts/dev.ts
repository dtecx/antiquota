/** Owns the safe development entry point: fake agent, disposable data, isolated port. */
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { createSandbox } from './sandbox.ts';
import { FixtureEngine } from './engine.ts';

const sandbox = await createSandbox('antiquota-dev');
const engine = new FixtureEngine(sandbox);
await engine.start();
console.log(`Development data: ${sandbox.root}`);
console.log('Fake agent only. Browser UI: http://127.0.0.1:5173');
const ui = spawn(process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm', ['dev'], {
  cwd: join(sandbox.repository, 'ui'),
  stdio: 'inherit',
  shell: process.platform === 'win32',
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
  ui.kill('SIGTERM');
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
