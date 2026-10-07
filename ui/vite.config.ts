/** Owns development transport and the stable production asset paths. */
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'node:fs';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [
    svelte(),
    {
      name: 'antiquota-token',
      apply: 'serve',
      transformIndexHtml(html) {
        const tokenPath = process.env.ANTIQUOTA_DEV_TOKEN_FILE;
        if (!tokenPath) return html;
        const token = readFileSync(tokenPath, 'utf8').trim();
        if (!/^[a-zA-Z0-9_-]{32,}$/.test(token)) throw new Error('Invalid development token');
        return html.replace('__ANTIQUOTA_TOKEN__', token);
      },
    },
  ],
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
    proxy: {
      '/api': {
        target: process.env.ANTIQUOTA_DEV_ENGINE_URL || 'http://127.0.0.1:8766',
        changeOrigin: true,
      },
    },
  },
  build: {
    cssCodeSplit: false,
    rolldownOptions: {
      output: { entryFileNames: 'app.js', assetFileNames: 'app[extname]' },
    },
  },
});
