/** Proves authenticated route requests and recoverable error messages. */
import { describe, expect, it, vi } from 'vitest';
import { createApi, RequestError } from './api';
import {
  fixtureAccount,
  fixtureSettings,
  fixtureSession,
  fixtureState,
} from './fixtures.test-support';
describe('typed API boundary', () => {
  it('sends the token and exact JSON request through the captured route', async () => {
    const request = vi.fn<typeof fetch>().mockResolvedValue(Response.json(fixtureSession('a')));
    const api = createApi('synthetic-token', request);
    expect((await api.create({ cwd: '/Desktop/test/project', title: 'A' })).id).toBe('a');
    expect(request).toHaveBeenCalledWith('/api/sessions', {
      method: 'POST',
      headers: { 'X-Antiquota-Token': 'synthetic-token', 'Content-Type': 'application/json' },
      body: JSON.stringify({ cwd: '/Desktop/test/project', title: 'A' }),
    });
  });
  it('lists sessions through state and encodes identifiers and the SSE token', async () => {
    const request = vi
      .fn<typeof fetch>()
      .mockImplementation(async () => Response.json(fixtureState()));
    const api = createApi('a+b=', request);
    await api.state();
    await api.session('a/b');
    expect(request.mock.calls.map((call) => call[0])).toEqual([
      '/api/state',
      '/api/sessions/a%2Fb',
    ]);
    expect(api.streamUrl()).toBe('/api/events?token=a%2Bb%3D');
  });
  it('surfaces engine errors, malformed responses, and lost connections', async () => {
    const request = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(Response.json({ error: 'Folder does not exist.' }, { status: 400 }))
      .mockResolvedValueOnce(new Response('bad json'))
      .mockRejectedValueOnce(new TypeError('offline'));
    const api = createApi('synthetic', request);
    await expect(api.state()).rejects.toThrow('Folder does not exist.');
    await expect(api.state()).rejects.toThrow('empty response');
    await expect(api.state()).rejects.toThrow('Your draft is kept');
  });
  it('refuses a missing bootstrap token before any network request', async () => {
    const request = vi.fn<typeof fetch>();
    await expect(createApi('__ANTIQUOTA_TOKEN__', request).state()).rejects.toBeInstanceOf(
      RequestError,
    );
    expect(request).not.toHaveBeenCalled();
  });
  it('uses exact account, login, routing and defaults routes with encoded identities', async () => {
    const request = vi
      .fn<typeof fetch>()
      .mockImplementation(async () => Response.json({ ok: true }));
    const api = createApi('synthetic-token', request),
      name = 'one+slot@example.com';
    await api.accounts();
    await api.probeAccounts();
    await api.pauseAccount(name, { on: false });
    await api.checkAccount(name);
    await api.removeAccount(name);
    await api.startLogin();
    await api.loginState('id/+');
    await api.submitLogin('id/+', { code: 'synthetic-code' });
    await api.cancelLogin('id/+');
    await api.settings();
    await api.saveDefaults({ reserve: '0', quota_poll: '240' });
    await api.routing();
    expect(
      request.mock.calls.map(([path, options]) => [path, options?.method, options?.body]),
    ).toEqual([
      ['/api/accounts', 'GET', undefined],
      ['/api/accounts/probe', 'POST', '{}'],
      ['/api/accounts/one%2Bslot%40example.com/pause', 'POST', '{"on":false}'],
      ['/api/accounts/one%2Bslot%40example.com/check', 'POST', '{}'],
      ['/api/accounts/one%2Bslot%40example.com', 'DELETE', undefined],
      ['/api/accounts/login', 'POST', '{}'],
      ['/api/accounts/login/id%2F%2B', 'GET', undefined],
      ['/api/accounts/login/id%2F%2B', 'POST', '{"code":"synthetic-code"}'],
      ['/api/accounts/login/id%2F%2B', 'DELETE', undefined],
      ['/api/settings', 'GET', undefined],
      ['/api/settings/defaults', 'PUT', '{"reserve":"0","quota_poll":"240"}'],
      ['/api/routing?n=60', 'GET', undefined],
    ]);
    for (const [, options] of request.mock.calls)
      expect(options?.headers).toMatchObject({ 'X-Antiquota-Token': 'synthetic-token' });
  });
  it('returns typed account and settings data without transforming quota windows or strings', async () => {
    const account = fixtureAccount(),
      settings = fixtureSettings();
    const request = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(Response.json([account]))
      .mockResolvedValueOnce(Response.json(settings));
    const api = createApi('synthetic', request);
    expect(await api.accounts()).toEqual([account]);
    expect(await api.settings()).toEqual(settings);
  });
  it('uploads raw attachments and recordings with their headers and builds media URLs', async () => {
    const request = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        Response.json({ name: 'a b.png', path: '/x/a b.png', image: true, size: 3 }),
      )
      .mockResolvedValueOnce(Response.json({ text: 'Hi.', slot: 'one@example.com' }));
    const api = createApi('t+k', request);
    const file = new Blob(['abc']);
    expect((await api.attach('s/1', file, 'a b.png')).size).toBe(3);
    const audio = new Blob(['RIFF'], { type: 'audio/wav' });
    expect(await api.transcribe(audio, 'wav')).toEqual({ text: 'Hi.', slot: 'one@example.com' });
    expect(request.mock.calls).toEqual([
      [
        '/api/sessions/s%2F1/attach',
        {
          method: 'POST',
          headers: { 'X-Antiquota-Token': 't+k', 'X-Filename': 'a%20b.png' },
          body: file,
        },
      ],
      [
        '/api/transcribe',
        {
          method: 'POST',
          headers: { 'X-Antiquota-Token': 't+k', 'X-Audio-Type': 'wav' },
          body: audio,
        },
      ],
    ]);
    expect(api.attachmentUrl('s/1', 'a b.png')).toBe(
      '/api/sessions/s%2F1/attachments/a%20b.png?token=t%2Bk',
    );
  });
  it('encodes all recovery, discovery and stack route parameters without changing bodies', async () => {
    const request = vi
      .fn<typeof fetch>()
      .mockImplementation(async () => Response.json({ ok: true }));
    const api = createApi('synthetic', request);
    await api.search('a+b &');
    await api.fork('a/b');
    await api.clear('a/b');
    await api.compact('a/b');
    await api.rewind('a/b', 3);
    await api.inherit('a/b', 'source/+');
    await api.stateDocument('a/b');
    await api.shell('a/b', 'printf hi');
    await api.note('a/b', 'Keep this.');
    await api.mentionFiles('a/b', 'a &');
    await api.engine();
    await api.engineLog();
    await api.usage(30);
    await api.browse('/a b');
    await api.stackFile('config/a b.json');
    await api.writeStackFile('config/x', '{}');
    await api.stackItem('mcp', 'a/b');
    await api.putStackItem('hook', 'a/b', { enabled: false });
    await api.deleteStackItem('skill', 'a/b');
    await api.enableStackItem('plugin', 'a/b', false);
    await api.copyStackItem('ide', 'skill', 'a/b');
    await api.stack();
    await api.reseedStack();
    expect(
      request.mock.calls.map(([path, options]) => [path, options?.method, options?.body]),
    ).toEqual([
      ['/api/search?q=a%2Bb+%26', 'GET', undefined],
      ['/api/sessions/a%2Fb/fork', 'POST', '{}'],
      ['/api/sessions/a%2Fb/clear', 'POST', '{}'],
      ['/api/sessions/a%2Fb/compact', 'POST', '{}'],
      ['/api/sessions/a%2Fb/rewind', 'POST', '{"turn":3}'],
      ['/api/sessions/a%2Fb/inherit', 'POST', '{"from":"source/+"}'],
      ['/api/sessions/a%2Fb/state', 'GET', undefined],
      ['/api/sessions/a%2Fb/shell', 'POST', '{"cmd":"printf hi"}'],
      ['/api/sessions/a%2Fb/note', 'POST', '{"text":"Keep this."}'],
      ['/api/sessions/a%2Fb/files?q=a+%26', 'GET', undefined],
      ['/api/engine', 'GET', undefined],
      ['/api/engine/log', 'GET', undefined],
      ['/api/usage?days=30', 'GET', undefined],
      ['/api/fs?path=%2Fa+b', 'GET', undefined],
      ['/api/settings/file?path=config%2Fa+b.json', 'GET', undefined],
      ['/api/settings/file', 'PUT', '{"path":"config/x","text":"{}"}'],
      ['/api/settings/mcp/a%2Fb', 'GET', undefined],
      ['/api/settings/hook/a%2Fb', 'PUT', '{"value":{"enabled":false}}'],
      ['/api/settings/skill/a%2Fb', 'DELETE', undefined],
      ['/api/settings/plugin/a%2Fb/enabled', 'POST', '{"on":false}'],
      ['/api/settings/copy', 'POST', '{"source":"ide","kind":"skill","name":"a/b"}'],
      ['/api/stack?refresh=1', 'GET', undefined],
      ['/api/stack/reseed', 'POST', '{}'],
    ]);
    await expect(api.stackItem('unsupported', 'name')).rejects.toThrow('not available');
    expect(request).toHaveBeenCalledTimes(23);
  });
});
