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
});
