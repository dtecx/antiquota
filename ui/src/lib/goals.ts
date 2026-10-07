/** Owns goal and plan presentation rules and the goal dialog's draft. */
import { writable } from 'svelte/store';
import type { EventRecord } from '../gen/EventRecord';
import type { Goal } from '../gen/Goal';
import type { GoalRequest } from '../gen/GoalRequest';
import type { Session } from '../gen/Session';

export const EXECUTE_PLAN =
  'Go ahead: carry out the plan you just proposed, step by step. Check each step works, and finish with a short summary of what changed.';

/** The goal bar's second line: rounds or state, check and verifier model. */
export function goalMeta(goal: Goal, sessionModel: string): string {
  return [
    goal.state === 'active' ? `round ${goal.round}/${goal.max_rounds}` : goal.state,
    goal.check && `check: ${goal.check}`,
    goal.verifier && `verifier: ${goal.verifier_model || sessionModel}`,
  ]
    .filter(Boolean)
    .join('  ·  ');
}

/** A plan-mode session whose last turn after the last message finished successfully. */
export function planReady(session: Session | null, events: EventRecord[]): boolean {
  if (!session || session.mode !== 'plan' || session.status !== 'idle') return false;
  let lastEnd = -1;
  let lastUser = -1;
  events.forEach((event, index) => {
    if (event.t === 'turn_end') lastEnd = index;
    if (event.t === 'user') lastUser = index;
  });
  const end = events[lastEnd];
  return lastEnd > lastUser && end?.t === 'turn_end' && end.status === 'SUCCESS';
}

export interface GoalDraft {
  objective: string;
  check: string;
  verifier: boolean;
  verifier_model: string;
  max_rounds: string;
  error: string;
  saving: boolean;
}

/** A draft from the session's goal, or the composer text for a new one. */
export function draftFor(goal: Goal | null, composer: string, verifierModel: string): GoalDraft {
  return {
    objective: goal?.objective ?? composer.trim(),
    check: goal?.check ?? '',
    verifier: goal?.verifier ?? true,
    verifier_model: goal?.verifier_model || verifierModel,
    max_rounds: String(goal?.max_rounds || 20),
    error: '',
    saving: false,
  };
}

/** The request for a draft, or the reason it cannot be sent. */
export function goalRequest(draft: GoalDraft): GoalRequest | string {
  if (!draft.objective.trim()) return 'Describe the objective first.';
  const rounds = Number(draft.max_rounds);
  if (!Number.isInteger(rounds) || rounds < 1 || rounds > 500)
    return 'Rounds must be a whole number from 1 to 500.';
  return {
    objective: draft.objective.trim(),
    check: draft.check.trim(),
    verifier: draft.verifier,
    verifier_model: draft.verifier_model,
    max_rounds: rounds,
  };
}

export const goalDialog = writable<GoalDraft | null>(null);

/** Set when the user asks to revise a plan; the composer asks what should change. */
export const revising = writable(false);
