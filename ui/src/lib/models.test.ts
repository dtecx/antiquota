/** Proves family switches choose only concrete variants and preserve effort. */
import { expect, it } from 'vitest';
import { modelFamilies, pickFamily } from './models';
it('preserves effort across families and retains unknown IDs', () => {
  const models = [
    { id: 'a-low', label: 'A (Low)' },
    { id: 'a-high', label: 'A (High)' },
    { id: 'b-low', label: 'B (Low)' },
    { id: 'b-high', label: 'B (High)' },
    { id: 'c', label: 'C' },
  ];
  expect(pickFamily(models, 'a-low', 'B')).toBe('b-low');
  expect(pickFamily(models, 'c', 'B')).toBe('b-high');
  expect(pickFamily(models, 'b-high', 'C')).toBe('c');
  expect(modelFamilies(models, 'unknown').at(-1)).toEqual({
    name: 'unknown',
    variants: [{ id: 'unknown', effort: '' }],
  });
});
