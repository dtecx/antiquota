/** Owns transcript file, clipboard, rewind and denied-action retries for their initiating session. */
import { writeClipboard } from './native';
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
    reuse: async (text: string): Promise<boolean> => {
      const value = get(workbench.state),
        id = value.selected?.id,
        draft = id ? (value.drafts[id] ?? '') : '';
      if (!id || disposed || value.loading) return false;
      if (
        draft &&
        draft !== text &&
        !(await ask(
          'Replace unsent draft?',
          'Load this earlier prompt in place of your current text. Attached files in the composer stay.',
          { ok: 'Replace draft' },
        ))
      )
        return false;
      const current = get(workbench.state);
      if (
        disposed ||
        current.loading ||
        current.selected?.id !== id ||
        (current.drafts[id] ?? '') !== draft
      )
        return false;
      workbench.draft(text, id);
      workbench.change({ feedback: 'Prompt loaded. Review text and attachments before sending.' });
      return true;
    },
    copy: async (text: string) => {
      try {
        await writeClipboard(text);
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
            return {
              ...result,
              ...(target.line
                ? { line: target.line, ...(target.endLine ? { endLine: target.endLine } : {}) }
                : {}),
            };
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
