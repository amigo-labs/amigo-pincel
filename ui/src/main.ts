import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';
import { initTheme } from './lib/theme.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('missing #app root element');

// The stored palette is in place before anything renders (amigo-theme AC-08).
await initTheme();

const app = mount(App, { target });

export default app;
