/** Owns typed, synthetic UI fixtures; no user accounts or files are used. */
import type { Session } from '../gen/Session';
import type { StateResponse } from '../gen/StateResponse';

export function fixtureSession(id = 'fixture'): Session {
  return {
    id,
    title: 'Fixture session',
    cwd: '/Desktop/test/project',
    model: 'gemini-3.8-flash-high',
    effort: 'high',
    mode: 'default',
    created: 100,
    updated: 100,
    status: 'idle',
    status_until: null,
    conversation_id: null,
    turns: 0,
    queue: [],
    goal: null,
    last_slot: null,
    usage: { input: 0, output: 0, thinking: 0 },
    context: 0,
    pending_note: '',
    jobs: [],
    loop: null,
    pinned: false,
    archived: false,
    worktree: null,
  };
}
export function fixtureState(sessions = [fixtureSession()]): StateResponse {
  return {
    sessions,
    accounts: [],
    models: [{ id: 'gemini-3.8-flash-high', label: 'Gemini Flash' }],
    modes: ['default'],
    defaults: {
      cwd: '/Desktop/test/project',
      default_cwd: '',
      model: 'gemini-3.8-flash-high',
      mode: 'default',
      verifier_model: 'gemini-3.8-flash-high',
      rotation: 'quota',
      keep_awake: 'off',
      reserve: '0',
      quota_poll: '60',
      approval_timeout: '300',
      max_rounds: '0',
      auto_continue: 'off',
      worktree_default: 'off',
    },
    engine: { version: 'fixture', agy: 'fake-agy', busy: 0 },
  };
}
