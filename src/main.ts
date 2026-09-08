import { mount } from 'svelte';

import App from './app.svelte';
import './styles/tokens.css';
import './styles/content.css';

/**
 * Nothing heavy is imported at module top level in this file. Ever.
 *
 * Mermaid, KaTeX, highlight.js and CodeMirror are reached only through
 * `import()`, gated on document content or on entering edit mode. A read-only
 * session on a plain document must download zero bytes of them — there is a
 * wdio assertion for exactly that.
 *
 * See .claude/skills/size-budget/SKILL.md.
 */

declare global {
  /** True in the lite build, where Mermaid is aliased to a stub. Set by Vite. */
  const __MARKLET_LITE__: boolean;

  interface Window {
    /**
     * HTML for the document the app was launched with, injected by Rust via
     * `initialization_script` before the window exists. Consuming it here means
     * the first paint already has content: no IPC round trip, no spinner.
     */
    __MARKLET_BOOT__?: { html: string; title: string };
  }
}

const doc = document.getElementById('doc');
const boot = window.__MARKLET_BOOT__;

if (doc && boot) {
  doc.innerHTML = boot.html;
  document.title = `${boot.title} — Marklet`;
}

mount(App, { target: document.getElementById('chrome')! });

// Show the window only once there is something to look at. The window is
// created `visible: false` precisely so the user never sees a white flash,
// which is what makes an app feel slow even when it is not.
requestAnimationFrame(() => {
  void import('@tauri-apps/api/window')
    .then(({ getCurrentWindow }) => getCurrentWindow().show())
    .catch(() => {
      /* running in a plain browser during `npm run dev` */
    });
});
