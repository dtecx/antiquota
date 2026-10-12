/** Keeps each session's viewport and text selection across pane changes in this window. */
export const scrollPositions = new Map<string, { top: number; follow: boolean }>();
/** Opens the conversation finder of the pane showing a session; set while it can search. */
export const finders = new Map<string, () => void>();
export const caretPositions = new Map<string, { start: number; end: number }>();
