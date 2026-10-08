/** Formats context and quota without making account routing decisions. */
import type { Session } from '../gen/Session';
import type { Account } from '../gen/Account';
import {
  accountSummary,
  minimumQuota,
  modelPool,
  quotaPercent,
  shortAccount,
} from './account-format';
import { formatTokens } from './format';
export function contextCapacity(model: string) {
  return model.startsWith('claude') ? 200_000 : model.startsWith('gpt-oss') ? 131_072 : 1_048_576;
}
export function sessionMeter(session: Session, accounts: Account[]) {
  const fraction = Math.min(1, Math.max(0, session.context / contextCapacity(session.model)));
  const pool = modelPool(session.model),
    serving = accounts.find((a) => a.slot === session.last_slot) ?? accounts[0];
  return {
    fraction,
    heat: fraction >= 0.8 ? 'hot' : fraction >= 0.5 ? 'warm' : '',
    context: `${Math.round(fraction * 100)}% context`,
    tokens: `${formatTokens(session.usage.input)} in / ${formatTokens(session.usage.output)} out / ${formatTokens(session.usage.thinking)} thinking`,
    account: serving
      ? `${shortAccount(serving.slot)} · ${pool === 'cg' ? 'Claude/GPT' : 'Gemini'} ${quotaPercent(minimumQuota(serving.quota[pool]))}`
      : 'No account',
    available: accountSummary(accounts, session.last_slot, pool).available,
  };
}
