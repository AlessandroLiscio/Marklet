import { fileURLToPath } from 'node:url';

import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

/**
 * `MARKLET_LITE=1` produces the lite artifact by aliasing Mermaid to a stub, so
 * the chunk is never emitted — roughly 750 KB off the installer.
 *
 * This lives here rather than behind a cargo feature because the 750 KB are in
 * the frontend bundle. A cargo feature would have gated nothing at all.
 */
const LITE = process.env.MARKLET_LITE === '1';

/**
 * Chunk boundaries are a size-budget decision, not a bundling detail.
 *
 * Everything heavy must land in its OWN chunk, reachable only through a dynamic
 * `import()` that is gated on document content. A read-only session that opens a
 * plain markdown file must download ZERO of these chunks.
 *
 * Budget (xz-compressed, as charged by the NSIS/LZMA installer):
 *   mermaid     ~750 KB   loaded only when /```mermaid/ matches
 *   codemirror  ~180 KB   loaded only on F2 / F3 / Ctrl+E
 *   katex       ~200 KB   loaded only when math delimiters match
 *   hljs         ~28 KB   loaded post-paint via IntersectionObserver
 *
 * See .claude/skills/size-budget/SKILL.md before touching manualChunks.
 */
export default defineConfig({
  plugins: [svelte()],

  define: {
    __MARKLET_LITE__: JSON.stringify(LITE),
  },

  resolve: {
    alias: LITE
      ? { mermaid: fileURLToPath(new URL('./src/lib/rich/mermaid-stub.ts', import.meta.url)) }
      : {},
  },

  // Tauri expects a fixed port and fails if it is taken.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },

  build: {
    // Windows 11 WebView2 and WebKitGTK 2.4x both handle modern syntax.
    target: 'es2022',
    // Vite 8 runs on Rolldown; oxc is its native minifier. 'esbuild' would
    // require a package Vite no longer bundles.
    minify: 'oxc',
    // Source maps are dev-only; they would otherwise ship in the installer.
    sourcemap: false,
    chunkSizeWarningLimit: 200,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/mermaid')) return 'mermaid';
          if (id.includes('node_modules/katex')) return 'katex';
          if (id.includes('node_modules/highlight.js')) return 'hljs';
          if (id.includes('node_modules/@codemirror') || id.includes('node_modules/@lezer')) {
            return 'codemirror';
          }
          return undefined;
        },
      },
    },
  },
});
