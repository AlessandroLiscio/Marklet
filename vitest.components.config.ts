import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

/**
 * Svelte components, mounted for real.
 *
 * `vitest.config.ts` runs plain TypeScript under Node and cannot compile a
 * `.svelte` file, which is how the keyboard sheet shipped unable to render: two
 * rows shared a key, Svelte rejected the keyed loop at mount, and nothing in the
 * suite ever mounted it. A component that is only ever rendered by the running
 * app is a component nobody has run.
 *
 * Both configs run from `npm test`. This one compiles with the Svelte plugin,
 * mounts into happy-dom, and defines `__MARKLET_EDITION__` the way
 * `vite.config.ts` does — `full` by default, `MARKLET_EDITION=lite` for the
 * other product — so the full-only branches are exercised unless asked
 * otherwise. Anything that talks to Rust is mocked at `src/lib/ipc`.
 */
export default defineConfig({
  plugins: [svelte()],
  define: {
    __MARKLET_EDITION__: JSON.stringify(process.env.MARKLET_EDITION === 'lite' ? 'lite' : 'full'),
  },
  // Svelte's browser build, not the server one: `mount` does not exist in the latter.
  resolve: { conditions: ['browser'] },
  test: {
    include: ['tests/components/**/*.test.ts'],
    environment: 'happy-dom',
    passWithNoTests: false,
  },
});
