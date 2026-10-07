/** Owns runtime guards for Rust-generated wire types at the untrusted stream boundary. */
import type { Account } from '../gen/Account';
import type { Session } from '../gen/Session';
import type { StreamEvent } from '../gen/StreamEvent';

const object = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === 'object' && !Array.isArray(value);
const number = (value: unknown): value is number =>
  typeof value === 'number' && Number.isFinite(value);
const strings = (value: Record<string, unknown>, keys: string[]) =>
  keys.every((key) => typeof value[key] === 'string');
const numbers = (value: Record<string, unknown>, keys: string[]) =>
  keys.every((key) => number(value[key]));
export function isAccount(value: unknown): value is Account {
  if (
    !object(value) ||
    typeof value.slot !== 'string' ||
    typeof value.busy !== 'boolean' ||
    typeof value.paused !== 'boolean'
  )
    return false;
  if (value.probed !== null && !number(value.probed)) return false;
  if (value.verify !== null && typeof value.verify !== 'string') return false;
  if (!object(value.quota) || !object(value.walls) || !object(value.why) || !object(value.today))
    return false;
  if (
    !Object.values(value.walls).every(number) ||
    !Object.values(value.why).every((reason) => typeof reason === 'string')
  )
    return false;
  if (
    !['input', 'output', 'thinking', 'turns'].every(
      (key) =>
        value.today &&
        object(value.today) &&
        (value.today[key] === undefined || number(value.today[key])),
    )
  )
    return false;
  return Object.values(value.quota).every(
    (pool) =>
      object(pool) &&
      Object.values(pool).every(
        (window) =>
          Array.isArray(window) &&
          window.length === 2 &&
          number(window[0]) &&
          (window[1] === null || number(window[1])),
      ),
  );
}
export function isSession(value: unknown): value is Session {
  if (
    !object(value) ||
    !strings(value, ['id', 'title', 'cwd', 'model', 'effort', 'mode', 'status', 'pending_note'])
  )
    return false;
  if (!numbers(value, ['created', 'updated', 'turns', 'context'])) return false;
  if (!['default', 'accept-edits', 'plan', 'bypass'].includes(String(value.mode))) return false;
  if (typeof value.pinned !== 'boolean' || typeof value.archived !== 'boolean') return false;
  return (
    Array.isArray(value.queue) &&
    Array.isArray(value.jobs) &&
    object(value.usage) &&
    numbers(value.usage, ['input', 'output', 'thinking'])
  );
}
export function isStreamEvent(value: unknown): value is StreamEvent {
  if (!object(value) || !number(value.ts) || (value.sid !== null && typeof value.sid !== 'string'))
    return false;
  const step = () => numbers(value, ['turn', 'step']);
  switch (value.t) {
    case 'meta':
      return isSession(value.meta);
    case 'user':
      return (
        strings(value, ['text', 'kind']) &&
        (value.attachments === undefined ||
          (Array.isArray(value.attachments) &&
            value.attachments.every(
              (item: unknown) =>
                object(item) && strings(item, ['name', 'path']) && typeof item.image === 'boolean',
            )))
      );
    case 'turn_start':
      return numbers(value, ['turn']) && strings(value, ['slot', 'model']);
    case 'delta':
    case 'assistant':
      return step() && strings(value, ['text']);
    case 'thinking':
      return step() && strings(value, ['text']) && number(value.tokens);
    case 'tool':
      return (
        step() &&
        strings(value, ['name', 'state']) &&
        'params' in value &&
        (value.output === undefined || typeof value.output === 'string') &&
        (value.diff === undefined || typeof value.diff === 'string') &&
        (value.secs === undefined || number(value.secs))
      );
    case 'turn_end':
      return (
        numbers(value, ['turn', 'secs']) &&
        strings(value, ['slot', 'status']) &&
        object(value.usage) &&
        numbers(value.usage, [
          'input_tokens',
          'output_tokens',
          'thinking_tokens',
          'total_tokens',
          'context',
        ])
      );
    case 'notice':
      return strings(value, ['kind', 'text']) && 'data' in value;
    case 'accounts':
      return Array.isArray(value.accounts) && value.accounts.every(isAccount);
    case 'approval':
      return (
        strings(value, ['id', 'session', 'tool', 'rule', 'state', 'why']) &&
        number(value.deadline) &&
        'args' in value
      );
    case 'job':
      return strings(value, ['phase']) && 'job' in value;
    case 'shell':
      return (
        strings(value, ['cmd', 'output']) &&
        number(value.secs) &&
        (value.code === null || number(value.code))
      );
    case 'deleted':
      return true;
    default:
      return false;
  }
}
