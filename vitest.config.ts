import { defineConfig } from 'vitest/config';

/**
 * Vitest covers pure TypeScript logic only: outline construction, the binary
 * search over the `data-l` line map, scroll math.
 *
 * Rendering correctness lives in `cargo test` against the golden fixtures,
 * because that is where the renderer is. Anything involving a real window is a
 * wdio + tauri-driver test — Playwright cannot attach to a webview embedded in
 * a Tauri window.
 */
export default defineConfig({
  test: {
    include: ['tests/unit/**/*.test.ts'],
    environment: 'node',
    passWithNoTests: true,
  },
});
