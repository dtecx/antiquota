/** Owns deterministic store, reducer, transport and formatting unit tests. */
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: { include: ['src/**/*.test.ts'], environment: 'node', maxWorkers: 2 },
});
