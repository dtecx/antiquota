/** Owns preprocessing for the shared browser and desktop Svelte build. */
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default { preprocess: vitePreprocess() };
