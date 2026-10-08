/** Owns transcript file, clipboard, rewind and denied-action retries for their initiating session. */
import { get } from 'svelte/store';
import type { Api } from './api';
import type { Workbench } from './workbench';
import { ask } from './confirm';
import { viewFile } from './git';
import { fileTarget } from './markdown';

export function createTranscriptActions(api: Api, workbench: Workbench) {
  let disposed = false;
  return {
    dispose: () => {
      disposed = true;
    },
    copy: async (text: string) => {
      try {
        await navigator.clipboard.writeText(text);
        workbench.change({ feedback: 'Copied' });
      } catch (error) {
        workbench.change({
          error: error instanceof Error ? error.message : 'Could not copy text.',
        });
      }
    },
    file: async (text: string) => {
      const id = get(workbench.state).selected?.id;
      if (!id || disposed) return;
      const target = fileTarget(text);
      await workbench.run(async () => {
        await viewFile(
          async () => {
            const result = await api.file(id, target.path);
            return { ...result, ...(target.line ? { line: target.line } : {}) };
          },
          () => !disposed && get(workbench.state).selected?.id === id,
        );
      });
    },
    rewind: (turn: number) => {
      const id = get(workbench.state).selected?.id;
      if (!id || !Number.isSafeInteger(turn) || turn < 1) return Promise.resolve();
      return workbench.run(async () => {
        if (
          !(await ask(
            `Rewind before turn ${turn}?`,
            'Restore the files the agent changed. Later user edits are preserved and may prevent rewind.',
            { danger: true, ok: 'Rewind' },
          ))
        )
          return;
        await api.rewind(id, turn);
        await workbench.refresh();
      });
    },
    allow: (rules: string[], retry: boolean) => {
      const id = get(workbench.state).selected?.id;
      if (!id || !rules.length) return Promise.resolve();
      const accepted = [...new Set(rules)];
      return workbench.run(async () => {
        await api.savePermissions({ add_allow: accepted });
        if (retry)
          await api.send(id, {
            text: `Permission granted (${accepted.join(', ')}). Retry what was denied and continue.`,
          });
        workbench.change({ feedback: `Allowed ${accepted.join(', ')}` });
      });
    },
    edits: () => {
      const id = get(workbench.state).selected?.id;
      if (!id) return Promise.resolve();
      return workbench.run(async () => {
        const session = await api.update(id, { mode: 'accept-edits' });
        workbench.acceptSession(session);
        await api.send(id, { text: 'Edits are now allowed. Retry what was denied and continue.' });
      });
    },
  };
}
export type TranscriptActions = ReturnType<typeof createTranscriptActions>;
export interface Expansion {
  version: number;
  open: boolean;
}
