/** Proves popup ordering, polling, timer cleanup and clipboard boundaries. */
import { get } from 'svelte/store';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createApi, type Api } from './api';
import { createLogin, type LoginBrowser } from './login';
function fixture() {
  const tab = { navigate: vi.fn(), close: vi.fn() };
  const browser: LoginBrowser = {
    reserve: vi.fn(() => tab),
    clipboard: vi.fn(async () => '4/abcdefghijklmnopqrstuv'),
  };
  const api: Api = {
    ...createApi('synthetic', vi.fn<typeof fetch>()),
    startLogin: vi.fn(async () => ({
      id: 'login-1',
      url: 'https://example.invalid/consent',
      seconds: 60,
    })),
    cancelLogin: vi.fn(async () => ({ ok: true })),
    loginState: vi.fn<Api['loginState']>(async () => ({
      state: 'waiting',
      error: null,
      name: null,
      verify_url: null,
    })),
    submitLogin: vi.fn<Api['submitLogin']>(async () => ({
      state: 'done',
      error: null,
      name: 'new@example.com',
      verify_url: null,
    })),
  };
  const refresh = vi.fn(async () => {});
  return { api, browser, tab, refresh, login: createLogin(api, refresh, browser) };
}
afterEach(() => vi.useRealTimers());
describe('account sign-in store', () => {
  it('reserves a tab before consent and cancels old login before requesting a new link', async () => {
    const { login, api, browser, tab } = fixture();
    login.open();
    await login.start();
    expect(vi.mocked(browser.reserve).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(api.startLogin).mock.invocationCallOrder[0] ?? 0,
    );
    expect(tab.navigate).toHaveBeenCalledWith('https://example.invalid/consent');
    login.code('old');
    await login.start();
    expect(api.cancelLogin).toHaveBeenCalledWith('login-1');
    expect(vi.mocked(api.cancelLogin).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(api.startLogin).mock.invocationCallOrder[1] ?? 0,
    );
    expect(get(login.state).code).toBe('');
    login.close();
  });
  it('offers a manual link when blocked and counts down through the last fifteen seconds', async () => {
    vi.useFakeTimers();
    const { login, browser } = fixture();
    browser.reserve = () => null;
    login.open();
    await login.start();
    expect(get(login.state).message).toContain('Open the Google');
    await vi.advanceTimersByTimeAsync(45_000);
    expect(get(login.state).seconds).toBe(15);
    await vi.advanceTimersByTimeAsync(15_000);
    expect(get(login.state).seconds).toBe(0);
    login.close();
    expect(vi.getTimerCount()).toBe(0);
  });
  it('polls every two seconds and closes completed sign-in after 1.2 seconds without a code', async () => {
    vi.useFakeTimers();
    const { login, api, refresh } = fixture();
    api.loginState = vi.fn<Api['loginState']>(async () => ({
      state: 'done',
      error: null,
      name: 'polled@example.com',
      verify_url: null,
    }));
    login.open();
    await login.start();
    await vi.advanceTimersByTimeAsync(1999);
    expect(api.loginState).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(get(login.state).phase).toBe('done');
    expect(refresh).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(1199);
    expect(get(login.state).open).toBe(true);
    await vi.advanceTimersByTimeAsync(1);
    expect(get(login.state).open).toBe(false);
    expect(api.cancelLogin).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
  });
  it('keeps verification and failed states with disabled code controls', async () => {
    const { login, api, refresh } = fixture();
    login.open();
    await login.start();
    api.submitLogin = vi.fn<Api['submitLogin']>(async () => ({
      state: 'verify',
      error: null,
      name: 'new@example.com',
      verify_url: 'https://example.invalid/verify',
    }));
    login.code('code');
    await login.submit();
    expect(get(login.state).phase).toBe('verify');
    expect(get(login.state).verify).toBe('https://example.invalid/verify');
    expect(refresh).toHaveBeenCalledOnce();
    await login.start();
    api.submitLogin = vi.fn<Api['submitLogin']>(async () => ({
      state: 'failed',
      error: 'Rejected',
      name: null,
      verify_url: null,
    }));
    login.code('bad');
    await login.submit();
    expect(get(login.state).phase).toBe('failed');
    expect(get(login.state).error).toContain('Get a new sign-in link');
    login.close();
  });
  it('restores code submission after errors and ignores double submit while checking', async () => {
    const { login, api } = fixture();
    login.open();
    await login.start();
    let reject: (error: Error) => void = () => {};
    api.submitLogin = vi.fn<Api['submitLogin']>(
      () =>
        new Promise((_resolve, fail) => {
          reject = fail;
        }),
    );
    login.code('good');
    const pending = login.submit();
    await login.submit();
    expect(api.submitLogin).toHaveBeenCalledOnce();
    expect(get(login.state).phase).toBe('submitting');
    reject(new Error('Offline'));
    await pending;
    expect(get(login.state).phase).toBe('waiting');
    expect(get(login.state).error).toBe('Offline');
    login.close();
  });
  it('cancels late consent after close without restoring stale state', async () => {
    const { login, api, tab } = fixture();
    let complete: (value: Awaited<ReturnType<Api['startLogin']>>) => void = () => {};
    api.startLogin = () =>
      new Promise((resolve) => {
        complete = resolve;
      });
    login.open();
    const pending = login.start();
    await Promise.resolve();
    login.close();
    complete({ id: 'late', url: 'https://example.invalid/late', seconds: 60 });
    await pending;
    expect(api.cancelLogin).toHaveBeenCalledWith('late');
    expect(tab.close).toHaveBeenCalled();
    expect(get(login.state).open).toBe(false);
  });
  for (const outcome of ['resolve', 'reject'] as const)
    it(`cancels consent when a deferred native opener ${outcome}s after close`, async () => {
      vi.useFakeTimers();
      const { login, api, tab, refresh } = fixture();
      let resolve: () => void = () => {};
      let reject: (error: Error) => void = () => {};
      tab.navigate.mockImplementation(
        () =>
          new Promise<void>((done, fail) => {
            resolve = done;
            reject = fail;
          }),
      );
      login.open();
      const pending = login.start();
      await Promise.resolve();
      await Promise.resolve();
      expect(tab.navigate).toHaveBeenCalledWith('https://example.invalid/consent');
      login.close();
      if (outcome === 'resolve') resolve();
      else reject(new Error('The native opener failed.'));
      await pending;
      await vi.advanceTimersByTimeAsync(65_000);
      expect(get(login.state)).toMatchObject({ open: false, id: '', phase: 'idle', error: '' });
      expect(api.cancelLogin).toHaveBeenCalledExactlyOnceWith('login-1');
      expect(tab.close).toHaveBeenCalled();
      expect(api.loginState).not.toHaveBeenCalled();
      expect(api.submitLogin).not.toHaveBeenCalled();
      expect(refresh).not.toHaveBeenCalled();
      expect(vi.getTimerCount()).toBe(0);
    });
  it('validates clipboard codes, tolerates denied access and preserves manual input', async () => {
    const { login, browser, api } = fixture();
    login.open();
    await login.start();
    browser.clipboard = async () => 'bad';
    await login.paste();
    expect(api.submitLogin).not.toHaveBeenCalled();
    expect(get(login.state).error).toContain('no sign-in code');
    browser.clipboard = async () => {
      throw new Error('denied');
    };
    await login.paste();
    expect(get(login.state).error).toContain('paste the code');
    login.code('manual');
    await login.paste();
    expect(get(login.state).code).toBe('manual');
    login.code('');
    browser.clipboard = async () => '4/abcdefghijklmnopqrstuv';
    await login.paste();
    expect(api.submitLogin).toHaveBeenCalledWith('login-1', { code: '4/abcdefghijklmnopqrstuv' });
    login.close();
  });
  it('closes a reserved popup and releases controls when starting sign-in fails', async () => {
    const { login, api, tab } = fixture();
    api.startLogin = vi.fn(async () => {
      throw new Error('agy unavailable');
    });
    login.open();
    await login.start();
    expect(tab.close).toHaveBeenCalled();
    expect(get(login.state).phase).toBe('idle');
    expect(get(login.state).error).toBe('agy unavailable');
    login.close();
  });
});
