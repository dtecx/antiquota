/** Owns browser bootstrap and the shared Svelte mount for webview and browser. */
import { mount } from 'svelte';
import App from './App.svelte';
import './styles/tokens.css';
import './styles/base.css';

const target = document.getElementById('app');
if (!target) throw new Error('The application mount is missing.');
mount(App, { target });
