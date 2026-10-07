/** Owns the isolated Rust engine lifecycle used by development and browser tests. */
import { spawn, type ChildProcess } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import { request } from 'node:http';
import { join } from 'node:path';
import type { Sandbox } from './sandbox.ts';

export class FixtureEngine {
  readonly sandbox: Sandbox;
  port = 0;
  token = '';
  child: ChildProcess | null = null;
  constructor(sandbox: Sandbox) {
    this.sandbox = sandbox;
  }
  async start(port = 0) {
    const log = createWriteStream(join(this.sandbox.root, 'server.log'), { flags: 'a' });
    const child = spawn(
      this.sandbox.executable('antiquota'),
      ['serve', '--port', String(port), '--no-open'],
      {
        cwd: this.sandbox.project,
        env: this.sandbox.env,
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    );
    this.child = child;
    child.stdout?.pipe(log, { end: false });
    child.stderr?.pipe(log, { end: false });
    child.once('close', () => log.end());
    let spawnError: Error | null = null;
    child.once('error', (error) => {
      spawnError = error;
    });
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
      if (spawnError) throw spawnError;
      if (child.exitCode !== null)
        throw new Error(
          `The fixture engine exited (${child.exitCode}). See ${this.sandbox.root}/server.log.`,
        );
      try {
        const discovery: unknown = JSON.parse(
          await readFile(join(this.sandbox.data, 'state', 'engine.json'), 'utf8'),
        );
        if (
          discovery &&
          typeof discovery === 'object' &&
          'port' in discovery &&
          typeof discovery.port === 'number'
        ) {
          this.port = discovery.port;
          this.token = (
            await readFile(join(this.sandbox.data, 'state', 'gui-token'), 'utf8')
          ).trim();
          if ((await this.call('GET', '/api/health')).status === 200) return;
        }
      } catch {
        /* Discovery can precede the listener becoming available. */
      }
      await new Promise((resolve) => setTimeout(resolve, 30));
    }
    await this.stop();
    throw new Error(
      `The fixture engine did not become ready. See ${this.sandbox.root}/server.log.`,
    );
  }
  call(method: string, path: string, body?: unknown): Promise<{ status: number; text: string }> {
    return new Promise((resolve, reject) => {
      const bytes = body === undefined ? '' : JSON.stringify(body);
      const call = request(
        {
          hostname: '127.0.0.1',
          port: this.port,
          path,
          method,
          headers: {
            'X-Antiquota-Token': this.token,
            'Content-Type': 'application/json',
            'Content-Length': Buffer.byteLength(bytes),
          },
        },
        (response) => {
          let text = '';
          response.setEncoding('utf8');
          response.on('data', (chunk: string) => {
            text += chunk;
          });
          response.on('end', () => resolve({ status: response.statusCode ?? 0, text }));
        },
      );
      call.setTimeout(1_000, () => call.destroy(new Error('Fixture request timed out.')));
      call.on('error', reject);
      call.end(bytes);
    });
  }
  async scenario(value: unknown) {
    await writeFile(this.sandbox.script, JSON.stringify(value), { mode: 0o600 });
  }
  async stop() {
    const child = this.child;
    this.child = null;
    if (!child || child.exitCode !== null) return;
    await this.call('POST', '/api/shutdown', {}).catch(() => {
      child.kill('SIGTERM');
    });
    const deadline = Date.now() + 3_000;
    while (child.exitCode === null && child.signalCode === null && Date.now() < deadline)
      await new Promise((resolve) => setTimeout(resolve, 20));
    if (child.exitCode === null && child.signalCode === null) {
      child.kill('SIGKILL');
      await new Promise<void>((resolve) => child.once('exit', () => resolve()));
    }
  }
  get url() {
    return `http://127.0.0.1:${this.port}`;
  }
}
