/** Clears only the accepted text, preserving drafts typed during an asynchronous send. */
export function acceptDraft(drafts: Record<string, string>, id: string, text: string) {
  return { ...drafts, [id]: drafts[id] === text ? '' : (drafts[id] ?? '') };
}
export function transferDraft(drafts: Record<string, string>, id: string, text: string) {
  return { ...acceptDraft(drafts, '', text), [id]: drafts[id] ?? text };
}
