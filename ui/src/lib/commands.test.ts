/** Proves the reference uses platform-specific modifier labels. */
import { expect, it } from 'vitest';
import { shortcutLabel } from './commands';
it('preserves Mac glyphs and spells Ctrl/Shift elsewhere', () => {
  expect(shortcutLabel('⌘⇧M', 'MacIntel')).toBe('⌘⇧M');
  expect(shortcutLabel('⌘⇧M', 'Win32')).toBe('Ctrl+Shift+M');
  expect(shortcutLabel('⌘= ⌘− ⌘0', 'Linux x86_64')).toBe('Ctrl+= Ctrl+− Ctrl+0');
});
