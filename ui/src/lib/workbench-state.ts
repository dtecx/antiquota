/** Shared view-state types for session selection, drafts and dialogs. */
import type { EventRecord } from '../gen/EventRecord';
import type { Session } from '../gen/Session';
import type { StateResponse } from '../gen/StateResponse';
export interface DialogState {
  id?: string | undefined;
  pendingText?: string | undefined;
  kind: 'create' | 'rename';
  title: string;
  cwd: string;
  worktree: boolean;
}
export interface WorkbenchState {
  snapshot: StateResponse | null;
  selected: Session | null;
  events: EventRecord[];
  drafts: Record<string, string>;
  filter: string;
  sidebarOpen: boolean;
  showArchived: boolean;
  dialog: DialogState | null;
  loading: boolean;
  mutating: boolean;
  error: string;
  feedback: string;
}
