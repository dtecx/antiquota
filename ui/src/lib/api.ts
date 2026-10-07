/** Owns authenticated HTTP requests and the single stream URL boundary. */
import type { CreateSessionRequest } from '../gen/CreateSessionRequest';
import type { PatchSessionRequest } from '../gen/PatchSessionRequest';
import type { SendRequest } from '../gen/SendRequest';
import type { SendResponse } from '../gen/SendResponse';
import type { Session } from '../gen/Session';
import type { SessionResponse } from '../gen/SessionResponse';
import type { StateResponse } from '../gen/StateResponse';
import type { OkResponse } from '../gen/OkResponse';
import type { ApiError } from '../gen/ApiError';
import { routes } from '../gen/routes';

export class RequestError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = 'RequestError';
  }
}

export function createApi(token: string, request: typeof fetch = fetch) {
  async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
    const catalogPath = path.replace(/\/sessions\/[^/]+/, '/sessions/{sid}');
    if (
      !routes.some(
        (route) =>
          route.method === method && route.path === catalogPath && route.status === 'implemented',
      )
    ) {
      throw new RequestError('This action is not available yet.', 501);
    }
    if (!token || token === '__ANTIQUOTA_TOKEN__') {
      throw new RequestError(
        'The engine did not provide a connection token. Open this UI from antiquota serve.',
        401,
      );
    }
    let response: Response;
    try {
      response = await request(path, {
        method,
        headers: {
          'X-Antiquota-Token': token,
          ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
        },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      });
    } catch {
      throw new RequestError('Cannot reach the engine. Your draft is kept here.', 0);
    }
    const value: unknown = await response.json().catch(() => null);
    if (!response.ok) {
      const error =
        value && typeof value === 'object' && 'error' in value && typeof value.error === 'string'
          ? (value as ApiError).error
          : `Request failed (${response.status}).`;
      throw new RequestError(error, response.status);
    }
    if (value === null)
      throw new RequestError('The engine returned an empty response.', response.status);
    return value as T;
  }
  const sessionPath = (id: string) => `/api/sessions/${encodeURIComponent(id)}`;
  return {
    state: () => call<StateResponse>('GET', '/api/state'),
    session: (id: string) => call<SessionResponse>('GET', sessionPath(id)),
    create: (body: CreateSessionRequest) => call<Session>('POST', '/api/sessions', body),
    update: (id: string, body: PatchSessionRequest) =>
      call<Session>('PATCH', sessionPath(id), body),
    send: (id: string, body: SendRequest) =>
      call<SendResponse>('POST', `${sessionPath(id)}/send`, body),
    stop: (id: string) => call<OkResponse>('POST', `${sessionPath(id)}/stop`, {}),
    streamUrl: () => `/api/events?token=${encodeURIComponent(token)}`,
  };
}

export type Api = ReturnType<typeof createApi>;
