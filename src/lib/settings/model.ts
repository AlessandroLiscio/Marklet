/**
 * Settings: pure logic, split out from `panel.svelte` for the same reason
 * `outline.ts` is split from `outline.svelte` — `vitest.config.ts` runs
 * plain `.ts` under `tests/unit/**`, not `.svelte` files.
 *
 * Mirrors `src-tauri/src/store.rs::Settings` field for field; see that
 * file's doc comment before changing either.
 */
import type { Settings } from '../ipc';
import { scrollToLine, visibleLine } from '../doc';

export const MEASURE_MIN = 48;
export const MEASURE_MAX = 100;

/** Mirrors `store::Settings::default()`. */
export const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  // tokens.css's own default (`--accent-hue: 221`). Kept in sync by eye with
  // store.rs's `Settings::default` — no build-time link between the two.
  accent_hue: 221,
  typeface: 'editorial',
  palette: 'default',
  density: 'normal',
  motion: 'on',
  measure: 68,
};

/**
 * Clamps a column-width request to the 48-100ch range the slider itself
 * already limits, and `store.rs::Settings::clamp` re-enforces on the Rust
 * side — belt and suspenders, since a value could reach this function from
 * somewhere other than the slider.
 */
export function clampMeasure(ch: number): number {
  return Math.min(MEASURE_MAX, Math.max(MEASURE_MIN, Math.round(ch)));
}

/** The subset of `HTMLElement` this module touches — lets tests pass a
 *  plain object instead of needing a real DOM (`vitest.config.ts` runs
 *  tests under Node, with no jsdom). */
export interface StyleTarget {
  setAttribute(name: string, value: string): void;
  removeAttribute(name: string): void;
  style: {
    setProperty(name: string, value: string): void;
    removeProperty(name: string): void;
  };
}

/**
 * Applies `settings` to `:root` as the data-attributes and custom properties
 * tokens.css and `src/styles/full/**` already know how to read:
 * `data-theme`, `data-density`, `data-motion` (tokens.css's own "Density
 * presets" / motion comments describe exactly this future control),
 * `data-typeface` / `data-palette` (full/typography.css, full/palettes.css),
 * plus the two live-adjustable custom properties `--measure` and
 * `--accent-hue`.
 *
 * `edition` gates typeface/palette so a lite build never sets attributes
 * whose CSS it does not ship — harmless either way, since lite's bundle
 * tree-shakes those selectors out entirely, but setting them anyway would be
 * a false appearance of support.
 */
export function applySettingsToRoot(
  root: StyleTarget,
  settings: Settings,
  edition: 'lite' | 'full'
): void {
  // Theme: absent attribute means "system", per tokens.css's three-state rule.
  if (settings.theme === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', settings.theme);

  if (settings.density === 'normal') root.removeAttribute('data-density');
  else root.setAttribute('data-density', settings.density);

  if (settings.motion === 'off') root.setAttribute('data-motion', 'off');
  else root.removeAttribute('data-motion');

  root.style.setProperty('--measure', `${clampMeasure(settings.measure)}ch`);

  if (edition === 'full' && settings.palette !== 'default') {
    // A named palette's CSS rule sets --accent-hue by selector
    // (`[data-palette="…"]`). An inline style on the same property always
    // wins over any selector, so the custom hue below must be CLEARED here —
    // leaving a stale inline value would silently override the palette the
    // user just picked.
    root.setAttribute('data-palette', settings.palette);
    root.style.removeProperty('--accent-hue');
  } else {
    root.removeAttribute('data-palette');
    root.style.setProperty('--accent-hue', String(settings.accent_hue));
  }

  if (edition === 'full') {
    root.setAttribute('data-typeface', settings.typeface);
  } else {
    root.removeAttribute('data-typeface');
  }
}

/**
 * Runs `apply` — a synchronous mutation that may reflow the document, e.g. a
 * typeface, density or measure change — while keeping the same source line
 * at the top of the viewport.
 *
 * Anchored to the nearest `data-l` via `doc.ts`'s `visibleLine` /
 * `scrollToLine`, never to a saved pixel offset: a pixel offset is exactly
 * what a font, density or column-width change invalidates, which is why
 * restoring position matters most right after one of these three changes.
 *
 * Two animation frames before restoring, not one: the first lets the browser
 * commit the style mutation, the second lets it actually settle the new
 * layout before `scrollToLine` reads anchor positions against it — the same
 * "two frames so layout has settled" reasoning `main.ts` uses before
 * showing the window.
 */
export function preserveScrollAcrossReflow(docRoot: HTMLElement, apply: () => void): void {
  const line = visibleLine(docRoot);
  apply();
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      scrollToLine(line);
    });
  });
}
