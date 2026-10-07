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

export function fixtureAccount(slot = 'one@example.com'): import('../gen/Account').Account {
  return {
    slot,
    busy: false,
    probed: 100,
    quota: { gemini: { '5h': [0.9, 200], weekly: [0.8, 300] }, cg: { '5h': [0.3, null] } },
    walls: {},
    why: {},
    paused: false,
    verify: null,
    today: { input: 12, output: 8, thinking: 2, turns: 1 },
  };
}
export function fixtureSettings(): import('../gen/SettingsOverview').SettingsOverview {
  const { cwd: _cwd, ...defaults } = fixtureState().defaults;
  void _cwd;
  return {
    path: '/Desktop/test/data/stack',
    skills: [],
    plugins: [],
    rules: [],
    hooks: [],
    mcps: [],
    gemini_md: { cli: null },
    permissions: { allow: [], deny: [], cli: '' },
    defaults: { ...defaults, rotation: 'perishable', reserve: '3', quota_poll: '10' },
  };
}
