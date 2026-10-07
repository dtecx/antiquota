/** Owns antiquota's allow/deny rule lists and the approval wait setting. */
import { get, writable } from 'svelte/store';
import type { Api } from './api';

export type RuleKind = 'allow' | 'deny';
export interface PermissionsView {
  allow: string[];
  deny: string[];
  timeout: string;
  loading: boolean;
  saving: boolean;
  error: string;
  feedback: string;
}

/** Why a typed rule cannot be saved, or an empty string. The engine checks again. */
export function ruleProblem(rule: string, existing: string[]): string {
  const text = rule.trim();
  if (!text) return 'Type a rule first.';
  if (text.length >= 300) return 'Rules must be shorter than 300 characters.';
  if (!/^(command\(.+\)|mcp\(.+\))$/.test(text))
    return 'Use command(git status) or mcp(server/tool).';
  if (existing.includes(text)) return 'That rule is already in the list.';
  return '';
}

export function createPermissions(api: Api) {
  const state = writable<PermissionsView>({
    allow: [],
    deny: [],
    timeout: '15',
    loading: false,
    saving: false,
    error: '',
    feedback: '',
  });
  const fail = (error: unknown) =>
    state.update((value) => ({
      ...value,
      error: error instanceof Error ? error.message : 'The change could not be saved.',
      feedback: '',
    }));
  async function refresh() {
    state.update((value) => ({ ...value, loading: true, error: '' }));
    try {
      const overview = await api.settings();
      state.update((value) => ({
        ...value,
        allow: overview.permissions.allow,
        deny: overview.permissions.deny,
        timeout: overview.defaults.approval_timeout,
      }));
    } catch (error) {
      fail(error);
    } finally {
      state.update((value) => ({ ...value, loading: false }));
    }
  }
  async function save(kind: RuleKind, rules: string[], feedback: string) {
    state.update((value) => ({ ...value, saving: true, error: '', feedback: '' }));
    try {
      const saved = await api.savePermissions({ [kind]: rules });
      state.update((value) => ({
        ...value,
        allow: saved.allow ?? value.allow,
        deny: saved.deny ?? value.deny,
        feedback,
      }));
      return true;
    } catch (error) {
      fail(error);
      return false;
    } finally {
      state.update((value) => ({ ...value, saving: false }));
    }
  }
  return {
    state,
    refresh,
    add: async (kind: RuleKind, rule: string) => {
      const current = get(state)[kind];
      const problem = ruleProblem(rule, current);
      if (problem) {
        state.update((value) => ({ ...value, error: problem, feedback: '' }));
        return false;
      }
      return save(kind, [...current, rule.trim()], `Added ${rule.trim()}.`);
    },
    remove: (kind: RuleKind, rule: string) =>
      save(
        kind,
        get(state)[kind].filter((item) => item !== rule),
        `Removed ${rule}.`,
      ),
    saveTimeout: async (text: string) => {
      const minutes = Number(text);
      if (!text.trim() || !Number.isFinite(minutes) || minutes < 1 || minutes > 240) {
        state.update((value) => ({
          ...value,
          error: 'Approval wait must be between 1 and 240 minutes.',
          feedback: '',
        }));
        return;
      }
      state.update((value) => ({ ...value, saving: true, error: '', feedback: '' }));
      try {
        const saved = await api.saveDefaults({ approval_timeout: text.trim() });
        state.update((value) => ({
          ...value,
          timeout: saved.approval_timeout,
          feedback: 'Approval wait saved.',
        }));
      } catch (error) {
        fail(error);
      } finally {
        state.update((value) => ({ ...value, saving: false }));
      }
    },
  };
}
export type PermissionsStore = ReturnType<typeof createPermissions>;
