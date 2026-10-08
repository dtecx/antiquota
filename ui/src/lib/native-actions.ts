/** Routes native menu actions through the active workbench without changing its draft. */
import { get } from 'svelte/store';
import type { Api } from './api';
import type { Workbench } from './workbench';
import { drawer, viewFile } from './git';
import { preferences } from './preferences';
import { shortcutsOpen } from './commands';
import { downloadSession } from './export';
import type { SettingsSection } from './settings';
export function zoomBy(delta: number) {
  const current = get(preferences.state).zoom;
  preferences.change({ zoom: delta === 0 ? 1 : Math.round((current + delta) * 100) / 100 });
}
export async function dispatchNative(
  action: string,
  api: Api,
  workbench: Workbench,
  host: {
    newSession: () => void;
    settings: (section: SettingsSection) => void;
    find: () => void;
    split: () => void;
    step: (delta: number) => void;
    select: (id: string) => Promise<void>;
  },
): Promise<void> {
  if (action.startsWith('session:') && action.slice(8).trim()) {
    await host.select(action.slice(8));
    return;
  }
  const session = get(workbench.state).selected;
  const pane = session
    ? document.getElementById(`prompt-${session.id}`)?.closest('.session-pane')
    : null;
  switch (action) {
    case 'settings':
      host.settings('general');
      break;
    case 'usage':
      host.settings('usage');
      break;
    case 'new':
      host.newSession();
      break;
    case 'export':
      if (session) {
        const durable = await api.session(session.id);
        await downloadSession(durable.meta, durable.events);
      }
      break;
    case 'find':
      host.find();
      break;
    case 'dictate':
      pane?.querySelector<HTMLButtonElement>('button.mic:not(:disabled)')?.click();
      break;
    case 'zoom_in':
      zoomBy(0.1);
      break;
    case 'zoom_out':
      zoomBy(-0.1);
      break;
    case 'zoom_reset':
      zoomBy(0);
      break;
    case 'panel':
      drawer.update((value) => ({ ...value, open: !value.open }));
      break;
    case 'split':
      host.split();
      break;
    case 'expand':
      pane?.querySelector<HTMLButtonElement>('.presentation button:last-child')?.click();
      break;
    case 'reload':
      location.reload();
      break;
    case 'prev_session':
      host.step(-1);
      break;
    case 'next_session':
      host.step(1);
      break;
    case 'shortcuts':
      shortcutsOpen.set(true);
      break;
    case 'engine_log':
      await viewFile(
        async () => ({ rel: 'engine log', path: 'engine.log', text: (await api.engineLog()).text }),
        () => true,
      );
      break;
  }
}
