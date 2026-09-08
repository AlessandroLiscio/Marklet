import { mount } from 'svelte';

import App from './app.svelte';
import { setDocument } from './lib/doc';
import type { IpcError, OpenedDocument } from './lib/ipc';
import './styles/tokens.css';
import './styles/content.css';

/**
 * The boot path, and the whole cold-start budget.
 *
 * **Nothing heavy is imported at module top level in this file. Ever.** Mermaid,
 * KaTeX, highlight.js and CodeMirror are reached only through `import()` gated
 * on document content or on entering edit mode, in *both* editions —
 * `npm run check:imports` fails the build otherwise. A read-only session on a
 * plain document must download zero bytes of them.
 *
 * See `.claude/skills/size-budget/SKILL.md` and `docs/editions.md`.
 */

// The full edition's curated typography and extra accent palettes. The
// condition is a compile-time constant, so this whole block — and the two
// stylesheets it reaches — is eliminated from the lite bundle rather than
// shipped and skipped.
if (__MARKLET_EDITION__ === 'full') {
  void import('./styles/full/palettes.css');
  void import('./styles/full/typography.css');
}

declare global {
  /**
   * Which product this bundle is. A compile-time constant, so a
   * `if (__MARKLET_EDITION__ === 'full')` block is eliminated from the lite
   * bundle rather than shipped and skipped. See docs/editions.md.
   */
  const __MARKLET_EDITION__: 'lite' | 'full';

  interface Window {
    /**
     * The document the app was launched with, rendered in Rust **before this
     * window existed** and injected through an initialization script. Reading
     * it here means the first paint already has content: no IPC round trip, no
     * spinner, no second layout.
     */
    __MARKLET_BOOT__?: OpenedDocument;
    /** Set instead when the launch file could not be opened. */
    __MARKLET_BOOT_ERROR__?: IpcError;
  }
}

const doc = document.getElementById('doc');
const boot = window.__MARKLET_BOOT__;
const bootError = window.__MARKLET_BOOT_ERROR__;

if (doc && boot) {
  setDocument(doc, boot);
  document.title = `${boot.title} — Marklet`;
} else if (doc && bootError) {
  // Rendered from the same place as a document so it inherits content.css and
  // is not a differently-styled surprise.
  doc.innerHTML =
    '<h1>Could not open that file</h1><p class="boot-error"></p><p class="boot-error-path"></p>';
  const message = doc.querySelector('.boot-error');
  const path = doc.querySelector('.boot-error-path');
  if (message) message.textContent = bootError.message;
  if (path && bootError.path) path.textContent = bootError.path;
  document.title = 'Marklet';
}

mount(App, { target: document.getElementById('chrome')! });

/**
 * Show the window only once there is something to look at.
 *
 * It is created hidden in `src-tauri/src/lib.rs` precisely so the user never
 * sees a white flash — which is what makes an app *feel* slow even when its
 * numbers are fine. Two frames, not one: the first commits the DOM mutation
 * above, the second lets layout and paint settle before the window appears.
 */
requestAnimationFrame(() => {
  requestAnimationFrame(() => {
    void import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().show())
      .catch(() => {
        /* running in a plain browser during `npm run dev` */
      });
  });
});
