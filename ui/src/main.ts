import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';
import { initLocale } from './lib/i18n/locale';
import { initTheme } from './lib/theme.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('missing #app root element');

// The stored palette (amigo-theme AC-08) and the UI language with its
// dictionary are in place before anything renders.
await Promise.all([initTheme(), initLocale()]);

const app = mount(App, { target });

export default app;
