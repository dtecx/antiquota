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
import type { Account } from '../gen/Account';
import type { LoginLink } from '../gen/LoginLink';
import type { LoginState } from '../gen/LoginState';
import type { LoginCodeRequest } from '../gen/LoginCodeRequest';
import type { PauseAccountRequest } from '../gen/PauseAccountRequest';
import type { DefaultsRequest } from '../gen/DefaultsRequest';
import type { Settings } from '../gen/Settings';
import type { SettingsOverview } from '../gen/SettingsOverview';
import type { RoutingEntry } from '../gen/RoutingEntry';
import type { ApprovalAnswerRequest } from '../gen/ApprovalAnswerRequest';
import type { ApprovalResponse } from '../gen/ApprovalResponse';
import type { PermissionsRequest } from '../gen/PermissionsRequest';
import type { GoalRequest } from '../gen/GoalRequest';
import type { JobStartRequest } from '../gen/JobStartRequest';
import type { LoopRequest } from '../gen/LoopRequest';
import type { LoopResponse } from '../gen/LoopResponse';
import type { Job } from '../gen/Job';
import type { JobLogResponse } from '../gen/JobLogResponse';
import type { FileText, Tree } from './git';
import type { TerminalInfo } from '../gen/TerminalInfo';
import type { Attachment } from '../gen/Attachment';
import type { TextResponse } from '../gen/TextResponse';
import type { StackFileResponse } from '../gen/StackFileResponse';
import type { NoteResponse } from '../gen/NoteResponse';
import type { FilesResponse } from '../gen/FilesResponse';
import type { FolderResponse } from '../gen/FolderResponse';
import type { SearchHit } from '../gen/SearchHit';
import type { UsageResponse } from '../gen/UsageResponse';
import type { StackResponse } from '../gen/StackResponse';
import type { EngineResponse } from '../gen/EngineResponse';
import type { EventRecord } from '../gen/EventRecord';
import type { JsonValue } from '../gen/serde_json/JsonValue';
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

/** The words dictation heard, and the account that transcribed them. */
export interface Transcript {
  text: string;
  slot: string;
}

export function createApi(
  token: string,
  request: typeof fetch = fetch,
  onError: (error: RequestError) => void = () => {},
) {
  function refuse(message: string, status: number): never {
    const error = new RequestError(message, status);
    try {
      onError(error);
    } catch {
      /* Feedback cannot change the HTTP failure. */
    }
    throw error;
  }
  async function call<T>(
    method: string,
    path: string,
    body?: unknown,
    raw?: { data: Blob; headers: Record<string, string> },
  ): Promise<T> {
    const catalogPath = (path.split('?')[0] ?? path)
      .replace(/\/accounts\/login\/[^/]+/, '/accounts/login/{lid}')
      .replace(/\/accounts\/(?!login(?:\/|$)|probe$)[^/]+/, '/accounts/{name}')
      .replace(/\/sessions\/[^/]+/, '/sessions/{sid}')
      .replace(/\/approvals\/[^/]+/, '/approvals/{aid}')
      .replace(/\/jobs\/[^/]+/, '/jobs/{jid}')
      .replace(/\/attachments\/[^/]+/, '/attachments/{name}')
      .replace(/\/settings\/(skill|plugin|rule|hook|mcp)\/[^/]+/, '/settings/{kind}/{name}');
    if (
      !routes.some(
        (route) =>
          route.method === method && route.path === catalogPath && route.status === 'implemented',
      )
    ) {
      refuse('This action is not available yet.', 501);
    }
    if (!token || token === '__ANTIQUOTA_TOKEN__') {
      refuse(
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
          ...(raw?.headers ?? {}),
          ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
        },
        ...(raw ? { body: raw.data } : body === undefined ? {} : { body: JSON.stringify(body) }),
      });
    } catch {
      refuse('Cannot reach the engine. Your draft is kept here.', 0);
    }
    const value: unknown = await response.json().catch(() => null);
    if (!response.ok) {
      const error =
        value && typeof value === 'object' && 'error' in value && typeof value.error === 'string'
          ? (value as ApiError).error
          : `Request failed (${response.status}).`;
      refuse(error, response.status);
    }
    if (value === null) refuse('The engine returned an empty response.', response.status);
    return value as T;
  }
  const sessionPath = (id: string) => `/api/sessions/${encodeURIComponent(id)}`;
  const accountPath = (name: string) => `/api/accounts/${encodeURIComponent(name)}`;
  const loginPath = (id: string) => `/api/accounts/login/${encodeURIComponent(id)}`;
  return {
    state: () => call<StateResponse>('GET', '/api/state'),
    shutdown: () => call<OkResponse>('POST', '/api/shutdown', {}),
    search: (q: string) => call<SearchHit[]>('GET', `/api/search?${new URLSearchParams({ q })}`),
    fork: (id: string) => call<Session>('POST', `${sessionPath(id)}/fork`, {}),
    clear: (id: string) => call<OkResponse>('POST', `${sessionPath(id)}/clear`, {}),
    compact: (id: string) => call<OkResponse>('POST', `${sessionPath(id)}/compact`, {}),
    rewind: (id: string, turn: number) =>
      call<FilesResponse>('POST', `${sessionPath(id)}/rewind`, { turn }),
    inherit: (id: string, from: string) =>
      call<OkResponse>('POST', `${sessionPath(id)}/inherit`, { from }),
    stateDocument: (id: string) => call<TextResponse>('GET', `${sessionPath(id)}/state`),
    shell: (id: string, cmd: string) =>
      call<EventRecord>('POST', `${sessionPath(id)}/shell`, { cmd }),
    note: (id: string, text: string) =>
      call<NoteResponse>('POST', `${sessionPath(id)}/note`, { text }),
    mentionFiles: (id: string, q: string) =>
      call<string[]>('GET', `${sessionPath(id)}/files?${new URLSearchParams({ q })}`),
    engine: () => call<EngineResponse>('GET', '/api/engine'),
    engineLog: () => call<TextResponse>('GET', '/api/engine/log'),
    usage: (days = 14, detail = false) =>
      call<UsageResponse>('GET', `/api/usage?days=${days}${detail ? '&detail=1' : ''}`),
    browse: (path: string) =>
      call<FolderResponse>('GET', `/api/fs?${new URLSearchParams({ path })}`),
    stackFile: (path: string) =>
      call<StackFileResponse>('GET', `/api/settings/file?${new URLSearchParams({ path })}`),
    writeStackFile: (path: string, text: string) =>
      call<OkResponse>('PUT', '/api/settings/file', { path, text }),
    stackItem: (kind: string, name: string) =>
      call<JsonValue>(
        'GET',
        `/api/settings/${encodeURIComponent(kind)}/${encodeURIComponent(name)}`,
      ),
    putStackItem: (kind: string, name: string, value: JsonValue) =>
      call<OkResponse>(
        'PUT',
        `/api/settings/${encodeURIComponent(kind)}/${encodeURIComponent(name)}`,
        { value },
      ),
    deleteStackItem: (kind: string, name: string) =>
      call<OkResponse>(
        'DELETE',
        `/api/settings/${encodeURIComponent(kind)}/${encodeURIComponent(name)}`,
      ),
    enableStackItem: (kind: string, name: string, on: boolean) =>
      call<OkResponse>(
        'POST',
        `/api/settings/${encodeURIComponent(kind)}/${encodeURIComponent(name)}/enabled`,
        { on },
      ),
    copyStackItem: (source: string, kind: string, name?: string) =>
      call<OkResponse>('POST', '/api/settings/copy', { source, kind, ...(name ? { name } : {}) }),
    stack: () => call<StackResponse>('GET', '/api/stack?refresh=1'),
    reseedStack: () => call<StackResponse>('POST', '/api/stack/reseed', {}),
    session: (id: string) => call<SessionResponse>('GET', sessionPath(id)),
    deleteSession: (id: string) => call<OkResponse>('DELETE', sessionPath(id)),
    create: (body: CreateSessionRequest) => call<Session>('POST', '/api/sessions', body),
    update: (id: string, body: PatchSessionRequest) =>
      call<Session>('PATCH', sessionPath(id), body),
    send: (id: string, body: SendRequest) =>
      call<SendResponse>('POST', `${sessionPath(id)}/send`, body),
    stop: (id: string) => call<OkResponse>('POST', `${sessionPath(id)}/stop`, {}),
    accounts: () => call<Account[]>('GET', '/api/accounts'),
    probeAccounts: () => call<OkResponse>('POST', '/api/accounts/probe', {}),
    pauseAccount: (name: string, body: PauseAccountRequest) =>
      call<Account[]>('POST', `${accountPath(name)}/pause`, body),
    checkAccount: (name: string) => call<Account[]>('POST', `${accountPath(name)}/check`, {}),
    removeAccount: (name: string) => call<OkResponse>('DELETE', accountPath(name)),
    startLogin: () => call<LoginLink>('POST', '/api/accounts/login', {}),
    loginState: (id: string) => call<LoginState>('GET', loginPath(id)),
    submitLogin: (id: string, body: LoginCodeRequest) =>
      call<LoginState>('POST', loginPath(id), body),
    cancelLogin: (id: string) => call<OkResponse>('DELETE', loginPath(id)),
    settings: () => call<SettingsOverview>('GET', '/api/settings'),
    saveDefaults: (body: DefaultsRequest) => call<Settings>('PUT', '/api/settings/defaults', body),
    routing: () => call<RoutingEntry[]>('GET', '/api/routing?n=60'),
    answerApproval: (id: string, body: ApprovalAnswerRequest) =>
      call<ApprovalResponse>('POST', `/api/approvals/${encodeURIComponent(id)}/answer`, body),
    setGoal: (id: string, body: GoalRequest) =>
      call<Session>('POST', `${sessionPath(id)}/goal`, body),
    resumeGoal: (id: string) => call<OkResponse>('POST', `${sessionPath(id)}/goal/resume`, {}),
    clearGoal: (id: string) => call<OkResponse>('DELETE', `${sessionPath(id)}/goal`),
    createJob: (id: string, body: JobStartRequest) =>
      call<Job>('POST', `${sessionPath(id)}/jobs`, body),
    setLoop: (id: string, body: LoopRequest) =>
      call<LoopResponse>('POST', `${sessionPath(id)}/loop`, body),
    stopJob: (id: string, jid: string) =>
      call<Job>('POST', `${sessionPath(id)}/jobs/${encodeURIComponent(jid)}/stop`, {}),
    jobLog: (id: string, jid: string) =>
      call<JobLogResponse>('GET', `${sessionPath(id)}/jobs/${encodeURIComponent(jid)}/log`),
    deleteLoop: (id: string) => call<OkResponse>('DELETE', `${sessionPath(id)}/loop`),
    gitStatus: (id: string) => call<unknown>('GET', `${sessionPath(id)}/git`),
    gitDiff: (id: string, path: string | null, committed: boolean) => {
      const query = new URLSearchParams();
      if (path) query.set('path', path);
      if (committed) query.set('committed', '1');
      return call<{ diff: string }>('GET', `${sessionPath(id)}/git/diff?${query}`);
    },
    gitCommit: (id: string, message: string) =>
      call<{ sha: string }>('POST', `${sessionPath(id)}/git/commit`, { message }),
    gitDiscard: (id: string, path: string) =>
      call<OkResponse>('POST', `${sessionPath(id)}/git/discard`, { path }),
    gitMerge: (id: string) => call<{ sha: string }>('POST', `${sessionPath(id)}/git/merge`, {}),
    gitRemove: (id: string, deleteBranch: boolean) =>
      call<Session>('POST', `${sessionPath(id)}/git/remove`, { delete_branch: deleteBranch }),
    termStart: (id: string, body: { cols: number; rows: number; restart: boolean }) =>
      call<TerminalInfo>('POST', `${sessionPath(id)}/term`, body),
    termInput: (id: string, data: string) =>
      call<OkResponse>('POST', `${sessionPath(id)}/term/input`, { data }),
    termResize: (id: string, cols: number, rows: number) =>
      call<OkResponse>('POST', `${sessionPath(id)}/term/resize`, { cols, rows }),
    termKill: (id: string) => call<OkResponse>('POST', `${sessionPath(id)}/term/kill`, {}),
    termStreamUrl: (id: string) =>
      `${sessionPath(id)}/term/stream?token=${encodeURIComponent(token)}`,
    tree: (id: string, path: string) =>
      call<Tree>('GET', `${sessionPath(id)}/tree?${new URLSearchParams({ path })}`),
    file: (id: string, path: string) =>
      call<FileText>('GET', `${sessionPath(id)}/file?${new URLSearchParams({ path })}`),
    attach: (id: string, data: Blob, name: string) =>
      call<Attachment>('POST', `${sessionPath(id)}/attach`, undefined, {
        data,
        headers: { 'X-Filename': encodeURIComponent(name) },
      }),
    /** Images and downloads load through elements, so the token travels in the query. */
    attachmentUrl: (id: string, name: string) =>
      `${sessionPath(id)}/attachments/${encodeURIComponent(name)}?token=${encodeURIComponent(token)}`,
    transcribe: (data: Blob, type: string) =>
      call<Transcript>('POST', '/api/transcribe', undefined, {
        data,
        headers: { 'X-Audio-Type': type },
      }),
    savePermissions: (body: PermissionsRequest) =>
      call<PermissionsRequest>('PUT', '/api/permissions', body),
    streamUrl: () => `/api/events?token=${encodeURIComponent(token)}`,
  };
}

export type Api = ReturnType<typeof createApi>;
