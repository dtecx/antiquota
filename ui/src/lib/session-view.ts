/** Keeps each session's viewport and text selection across pane changes in this window. */
export const scrollPositions = new Map<string, { top: number; follow: boolean }>();
export const caretPositions = new Map<string, { start: number; end: number }>();
