/** Owns local composer actions and slash dispatch; commands never become accidental prompts. */
import { get } from 'svelte/store';
import type { Api } from './api';
import type { Workbench } from './workbench';
import type { Mode } from '../gen/Mode';
import type { SettingsSection } from './settings';
import { ask } from './confirm';
import { draftFor, goalDialog } from './goals';
import { viewFile } from './git';
import { modelFamilies } from './models';
import { downloadSession } from './export';

export const INIT_PROMPT =
  'Study this project and write a concise GEMINI.md at its root for future agents: what it is, how it is laid out, how to build, run and test it, and the conventions to follow. If GEMINI.md or AGENTS.md already exists, improve it instead of starting over. Keep it under 80 lines.';
export const modes: Mode[] = ['default', 'accept-edits', 'plan', 'bypass'];
export async function confirmMode(mode: string): Promise<boolean> {
  if (!modes.includes(mode as Mode))
    throw new Error('Mode: default · accept-edits · plan · bypass');
  return (
    mode !== 'bypass' ||
    (await ask(
      'Allow bypass mode?',
      'Bypass ignores antiquota permission rules. agy can edit files and run commands without asking.',
      { ok: 'Use bypass', danger: true },
    ))
  );
}
export function createComposerActions(
  api: Api,
  workbench: Workbench,
  host: {
    settings: (section: SettingsSection) => void;
    login: () => void;
    help: () => void;
  },
) {
  const say = (feedback: string) => workbench.change({ feedback });
  async function changeMode(mode: string) {
    const id = get(workbench.state).selected?.id;
    if (!id) return;
    try {
      if (await confirmMode(mode)) await workbench.update({ mode: mode as Mode }, id);
    } catch (error) {
      workbench.change({ error: error instanceof Error ? error.message : String(error) });
    }
  }
  async function submit(attachments: string[] = []) {
    const value = get(workbench.state),
      session = value.selected;
    const id = session?.id ?? '',
      text = value.drafts[id] ?? '',
      trimmed = text.trim();
    if (value.mutating || value.loading) return false;
    if (!trimmed.startsWith('/') && !trimmed.startsWith('!') && !trimmed.startsWith('#')) {
      if (!session && trimmed) {
        workbench.openDialog('create', { pendingText: text });
        return false;
      }
      return workbench.send(attachments);
    }
    let accepted = false;
    await workbench.run(async () => {
      const need = () => {
        if (!session) throw new Error('Open a session first');
        return session;
      };
      const patch = async (body: Parameters<Api['update']>[1]) =>
        workbench.acceptSession(await api.update(need().id, body));
      if (trimmed.startsWith('!') || trimmed.startsWith('#')) {
        const arg = trimmed.slice(1).trim();
        if (!arg) throw new Error(trimmed[0] === '!' ? '! <command>' : '# <note>');
        need();
        if (trimmed[0] === '!') {
          await api.shell(id, arg);
          say('Command finished; output goes to the next prompt.');
        } else {
          const result = await api.note(id, arg);
          say(`Saved to ${result.path}`);
        }
        await workbench.refresh();
      } else {
        const cmd = (trimmed.split(/\s+/, 1)[0] ?? '').toLowerCase();
        const arg = trimmed.slice(cmd.length).trim();
        const section: Partial<Record<string, SettingsSection>> = {
          '/accounts': 'accounts',
          '/usage': 'usage',
          '/settings': 'general',
          '/permissions': 'permissions',
          '/skills': 'skills',
          '/hooks': 'hooks',
          '/mcp': 'mcp',
          '/stack': 'skills',
        };
        if (section[cmd]) host.settings(section[cmd]);
        else
          switch (cmd) {
            case '/help':
              host.help();
              break;
            case '/login':
              host.login();
              break;
            case '/new':
              workbench.openDialog('create');
              break;
            case '/quota':
              await api.probeAccounts();
              say('Probing accounts…');
              break;
            case '/goal': {
              const current = need();
              if (!arg)
                goalDialog.set(
                  draftFor(
                    current.goal,
                    '',
                    value.snapshot?.defaults.verifier_model || current.model,
                  ),
                );
              else {
                workbench.acceptSession(
                  await api.setGoal(id, {
                    objective: arg,
                    verifier: true,
                    verifier_model: value.snapshot?.defaults.verifier_model || current.model,
                  }),
                );
                say('Goal started');
              }
              break;
            }
            case '/clear':
              need();
              await api.clear(id);
              say('Fresh conversation from the next message');
              break;
            case '/compact':
              need();
              await api.compact(id);
              say('Compacted');
              break;
            case '/rewind': {
              need();
              const turn = Number(arg);
              if (!arg || !Number.isSafeInteger(turn) || turn < 1)
                throw new Error('Which turn? /rewind <turn number>');
              if (
                !(await ask(
                  `Rewind before turn ${turn}?`,
                  'Files return to their checkpoint. Later transcript entries stay.',
                  { ok: 'Rewind', danger: true },
                ))
              )
                return;
              const result = await api.rewind(id, turn);
              say(`Rewound ${result.files.length} files`);
              break;
            }
            case '/export': {
              need();
              const durable = await api.session(id);
              await downloadSession(durable.meta, durable.events);
              break;
            }
            case '/init':
              need();
              await api.send(id, { text: INIT_PROMPT + (arg ? `\n\n${arg}` : '') });
              break;
            case '/bg':
              need();
              if (!arg) say('/bg <command>');
              else {
                await api.createJob(id, { cmd: arg });
                say('Running in the background');
              }
              break;
            case '/loop':
            case '/schedule': {
              need();
              if (!arg || /^(off|stop)$/i.test(arg)) {
                await api.deleteLoop(id);
                say(cmd === '/loop' ? 'Loop stopped' : 'Schedule removed');
                break;
              }
              const [when = '', ...rest] = arg.split(/\s+/),
                prompt = rest.join(' ');
              const result = await api.setLoop(
                id,
                cmd === '/loop' ? { every: when, prompt } : { at: when, prompt },
              );
              say(
                cmd === '/loop'
                  ? `Every ${when}: ${prompt.slice(0, 60)}`
                  : `Every day at ${when}; next ${new Date((result.next ?? 0) * 1000).toLocaleString()}`,
              );
              break;
            }
            case '/memory': {
              need();
              await viewFile(
                async () => {
                  const file = await api
                    .file(id, 'GEMINI.md')
                    .catch(() => api.file(id, 'AGENTS.md'))
                    .catch(() => null);
                  if (!file && get(workbench.state).selected?.id === id)
                    say('No GEMINI.md yet: add a line with "# …" or run /init');
                  return file;
                },
                () => get(workbench.state).selected?.id === id,
              );
              break;
            }
            case '/state':
              need();
              await viewFile(
                async () => ({
                  rel: `handoff state · ${session?.title || id}`,
                  path: 'STATE.md',
                  text: (await api.stateDocument(id)).text,
                }),
                () => get(workbench.state).selected?.id === id,
              );
              break;
            case '/stop':
              need();
              await api.stop(id);
              break;
            case '/resume':
              need();
              await api.resumeGoal(id);
              break;
            case '/cleargoal':
              need();
              await api.clearGoal(id);
              break;
            case '/model':
              need();
              if (!arg) throw new Error('/model <id>');
              await patch({ model: arg });
              say(`Model: ${arg}`);
              break;
            case '/effort': {
              const current = need(),
                family = modelFamilies(value.snapshot?.models ?? [], current.model).find((f) =>
                  f.variants.some((v) => v.id === current.model),
                );
              const variant = family?.variants.find((v) => v.effort === arg.toLowerCase());
              if (!variant) say(`${family?.name || current.model} has no effort "${arg}"`);
              else {
                await patch({ model: variant.id });
                say(`Model: ${variant.id}`);
              }
              break;
            }
            case '/mode':
              need();
              if (!(await confirmMode(arg))) return;
              await patch({ mode: arg as Mode });
              say(`Mode: ${arg}`);
              break;
            case '/rename':
              need();
              if (!arg) throw new Error('/rename <title>');
              await patch({ title: arg });
              break;
            case '/delete': {
              const current = need();
              if (
                !(await ask(
                  `Delete "${current.title || 'Untitled'}"?`,
                  'The transcript and handoff state are deleted. Files in the project stay.',
                  { ok: 'Delete', danger: true },
                ))
              )
                return;
              await api.deleteSession(id);
              break;
            }
            default:
              throw new Error(`Unknown command ${cmd}`);
          }
      }
      accepted = true;
      if (get(workbench.state).drafts[id] === text) workbench.draft('', id);
    });
    // Stream metadata normally updates the pane; reconcile clear/stop/deletion immediately.
    if (
      accepted &&
      /^(?:!(?:\s|\S)|\/(?:clear|compact|rewind|stop|resume|cleargoal|delete|bg|loop|schedule)(?:\s|$))/i.test(
        trimmed,
      )
    )
      await workbench.refresh();
    return false;
  }
  return { submit, changeMode };
}

export type ComposerActions = ReturnType<typeof createComposerActions>;
