/** Owns settings drafts and private-stack editors, keeping refreshes away from typed text. */
import { get, writable } from 'svelte/store';
import type { Api } from './api';
import type { Settings } from '../gen/Settings';
import type { SettingsOverview } from '../gen/SettingsOverview';
import type { JsonValue } from '../gen/serde_json/JsonValue';
import { ask } from './confirm';
export const settingsSections = [
  ['general', 'General'],
  ['appearance', 'Appearance'],
  ['keyboard', 'Keyboard'],
  ['accounts', 'Accounts'],
  ['usage', 'Usage'],
  ['agents', 'Agents & goals'],
  ['permissions', 'Permissions'],
  ['rules', 'Rules'],
  ['skills', 'Skills'],
  ['hooks', 'Hooks'],
  ['mcp', 'MCP servers'],
  ['plugins', 'Plugins'],
  ['engine', 'Engine'],
] as const;
export type SettingsSection = (typeof settingsSections)[number][0];
export type StackKind = 'rule' | 'skill' | 'hook' | 'mcp' | 'plugin';
interface Editor {
  key: string;
  kind: StackKind | null;
  name: string;
  path: string;
  text: string;
  loading: boolean;
}
export function itemPath(kind: StackKind, name: string) {
  if (kind === 'hook') return 'config/hooks.json';
  if (kind === 'mcp') return 'config/mcp_config.json';
  return kind === 'rule' ? `config/rules/${name}` : `config/skills/${name}/SKILL.md`;
}
export function itemTemplate(kind: StackKind, name: string): string | JsonValue {
  if (kind === 'rule') return `---\ntrigger: always_on\n---\n\n# ${name.replace(/\.md$/, '')}\n\n`;
  if (kind === 'skill')
    return `---\nname: ${name}\ndescription: What it does and when agy should use it.\n---\n\n# ${name}\n\n`;
  return kind === 'hook'
    ? {
        PostToolUse: [{ matcher: '*', hooks: [{ type: 'command', command: '', timeout: 30 }] }],
        enabled: true,
      }
    : { command: '', args: [] };
}
export function validateItemName(name: string) {
  if (!/^[A-Za-z0-9._:@+-]{1,100}$/.test(name) || name.startsWith('.'))
    throw new Error('Name: 1–100 letters, digits and . _ : @ + -; no leading dot.');
  return name;
}
export function createSettings(api: Api, saved: () => void = () => {}) {
  const state = writable<{
    data: SettingsOverview | null;
    fields: Settings | null;
    editor: Editor | null;
    loading: boolean;
    saving: boolean;
    error: string;
    feedback: string;
  }>({
    data: null,
    fields: null,
    editor: null,
    loading: false,
    saving: false,
    error: '',
    feedback: '',
  });
  const dirty = new Set<keyof Settings>();
  let generation = 0,
    editorGeneration = 0;
  function accept(data: SettingsOverview, reading = generation) {
    if (reading !== generation) return;
    state.update((v) => {
      const fields = { ...data.defaults };
      for (const key of dirty) if (v.fields) fields[key] = v.fields[key];
      return { ...v, data, fields };
    });
  }
  async function refresh() {
    const own = ++generation;
    state.update((v) => ({ ...v, loading: true, error: '' }));
    try {
      const data = await api.settings();
      if (own !== generation) return;
      accept(data);
    } catch (error) {
      if (own === generation) fail(error);
    } finally {
      if (own === generation) state.update((v) => ({ ...v, loading: false }));
    }
  }
  function fail(error: unknown) {
    state.update((v) => ({
      ...v,
      error: error instanceof Error ? error.message : 'Settings could not be saved.',
    }));
  }
  let mutations = Promise.resolve(true);
  function mutate(action: () => Promise<void>) {
    const pending = mutations.then(async () => {
      state.update((v) => ({ ...v, saving: true, error: '', feedback: '' }));
      try {
        await action();
        return true;
      } catch (error) {
        fail(error);
        return false;
      } finally {
        state.update((v) => ({ ...v, saving: false }));
      }
    });
    mutations = pending;
    return pending;
  }
  function field(key: keyof Settings, text: string) {
    dirty.add(key);
    state.update((v) => ({ ...v, fields: v.fields ? { ...v.fields, [key]: text } : null }));
  }
  async function saveField(key: keyof Settings) {
    const draft = get(state).fields?.[key];
    const text = key === 'default_cwd' ? draft?.trim() : draft;
    if (text === undefined) return false;
    if (draft !== text) field(key, text);
    return mutate(async () => {
      const ranges: Partial<Record<keyof Settings, [number, number]>> = {
        max_rounds: [1, 500],
        auto_continue: [0, 10],
        approval_timeout: [1, 240],
      };
      const range = ranges[key];
      if (
        range &&
        (!text.trim() ||
          !Number.isFinite(Number(text)) ||
          Number(text) < range[0] ||
          Number(text) > range[1])
      )
        throw new Error(`Enter ${range[0]}–${range[1]} for ${key.replaceAll('_', ' ')}.`);
      const defaults = await api.saveDefaults({ [key]: text });
      generation++;
      if (get(state).fields?.[key] === text) dirty.delete(key);
      state.update((v) => ({
        ...v,
        fields: v.fields
          ? { ...v.fields, [key]: dirty.has(key) ? v.fields[key] : defaults[key] }
          : defaults,
        data: v.data ? { ...v.data, defaults } : null,
        feedback: 'Settings saved.',
        loading: false,
      }));
      saved();
    });
  }
  function closeEditor() {
    editorGeneration++;
    state.update((v) => ({ ...v, editor: null }));
  }
  async function edit(
    kind: StackKind | null,
    name: string,
    path = kind ? itemPath(kind, name) : 'GEMINI.md',
  ) {
    const own = ++editorGeneration;
    state.update((v) => ({
      ...v,
      error: '',
      editor: { key: `${kind ?? 'file'}:${name}`, kind, name, path, text: '', loading: true },
    }));
    try {
      const text =
        kind === 'hook' || kind === 'mcp'
          ? JSON.stringify(await api.stackItem(kind, name), null, 2)
          : (await api.stackFile(path)).text;
      if (own === editorGeneration)
        state.update((v) => ({ ...v, editor: v.editor ? { ...v.editor, text } : null }));
    } catch (error) {
      if (own === editorGeneration) fail(error);
    } finally {
      if (own === editorGeneration)
        state.update((v) => ({ ...v, editor: v.editor ? { ...v.editor, loading: false } : null }));
    }
  }
  function editorText(text: string) {
    state.update((v) => ({ ...v, editor: v.editor ? { ...v.editor, text } : null }));
  }
  async function saveEditor() {
    const editor = get(state).editor;
    if (!editor || editor.loading) return false;
    const own = editorGeneration;
    return mutate(async () => {
      if (editor.kind === 'hook' || editor.kind === 'mcp') {
        const value: JsonValue = JSON.parse(editor.text);
        if (!value || typeof value !== 'object' || Array.isArray(value))
          throw new Error('A hook or server must be a JSON object.');
        await api.putStackItem(editor.kind, editor.name, value);
      } else await api.writeStackFile(editor.path, editor.text);
      if (own === editorGeneration) closeEditor();
      state.update((v) => ({ ...v, feedback: `Saved ${editor.name}.` }));
      await refresh();
    });
  }
  async function create(kind: StackKind, name: string) {
    const own = editorGeneration;
    return mutate(async () => {
      name = validateItemName(name.trim());
      if (kind === 'rule' && !name.endsWith('.md')) name += '.md';
      validateItemName(name);
      const data = await api.settings();
      const key =
        kind === 'mcp' ? 'mcps' : (`${kind}s` as 'rules' | 'skills' | 'hooks' | 'plugins');
      if (data[key].some((row) => row.name === name && row.here))
        throw new Error(`${name} already exists. Edit it instead.`);
      const value = itemTemplate(kind, name);
      if (kind === 'hook' || kind === 'mcp') await api.putStackItem(kind, name, value);
      else await api.writeStackFile(itemPath(kind, name), value as string);
      await refresh();
      if (own === editorGeneration) await edit(kind, name);
    });
  }
  async function copy(source: string, kind: string, name?: string) {
    return mutate(async () => {
      await api.copyStackItem(source, kind, name);
      await refresh();
      state.update((v) => ({
        ...v,
        feedback: `Copied from ${source === 'cli' ? 'agy CLI' : 'IDE'}.`,
      }));
    });
  }
  async function copyAll(source: string, kind: StackKind, names: string[]) {
    return mutate(async () => {
      for (const name of names) await api.copyStackItem(source, kind, name);
      await refresh();
      state.update((v) => ({ ...v, feedback: `Copied ${names.length} differing items.` }));
    });
  }
  async function remove(kind: StackKind, name: string) {
    const own = editorGeneration;
    if (
      !(await ask(`Delete ${kind} "${name}" from antiquota?`, 'Your agy CLI and IDE keep theirs.', {
        ok: 'Delete',
        danger: true,
      }))
    )
      return false;
    return mutate(async () => {
      await api.deleteStackItem(kind, name);
      const editor = get(state).editor;
      if (own === editorGeneration && editor?.kind === kind && editor.name === name) closeEditor();
      await refresh();
    });
  }
  async function toggle(kind: StackKind, name: string, on: boolean) {
    return mutate(async () => {
      await api.enableStackItem(kind, name, on);
      await refresh();
    });
  }
  return {
    state,
    accept,
    reading: () => generation,
    refresh,
    field,
    saveField,
    edit,
    closeEditor,
    editorText,
    saveEditor,
    create,
    copy,
    copyAll,
    remove,
    toggle,
    fail,
    dispose: () => {
      generation++;
      closeEditor();
    },
  };
}
export type SettingsStore = ReturnType<typeof createSettings>;
