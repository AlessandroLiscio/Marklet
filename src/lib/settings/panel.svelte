<script lang="ts">
  /**
   * The settings panel — inside the main window in both editions this wave
   * (per `docs/editions.md`, full's *dedicated window* is a later phase, not
   * this one). Every control here writes through to `store.rs` via the
   * `read_settings` / `write_settings` commands in `ipc.ts`.
   *
   * Self-contained like `outline.svelte`: not wired into `app.svelte` this
   * wave (main thread does that at wave close), so it resolves `#doc` and
   * `:root` itself rather than requiring props nobody passes yet.
   *
   * Typeface and palette are full-edition-only controls, eliminated from the
   * lite bundle at compile time by `if (__MARKLET_EDITION__ === 'full')` —
   * see `docs/editions.md` and `.claude/skills/size-budget/SKILL.md`.
   */
  import { onMount } from 'svelte';
  import { readSettings, writeSettings } from '../ipc';
  import type { Density, Palette, Settings, Theme, Typeface } from '../ipc';
  import { applySettingsToRoot, clampZoom, DEFAULT_SETTINGS, MEASURE_MAX, MEASURE_MIN, preserveScrollAcrossReflow, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP } from './model';

  const EDITION: 'lite' | 'full' = __MARKLET_EDITION__;

  // Opened and closed by the activity bar. `marklet --settings` still opens it
  // with the window rather than a moment after — `app.svelte` reads the same
  // boot flag and chooses the initial panel before the first paint.
  let { open = false }: { open?: boolean } = $props();
  let settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  let loaded = $state(false);

  const THEMES: { value: Theme; label: string }[] = [
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
    { value: 'system', label: 'System' },
  ];

  const DENSITIES: { value: Density; label: string }[] = [
    { value: 'compact', label: 'Compact' },
    { value: 'normal', label: 'Normal' },
    { value: 'spacious', label: 'Spacious' },
  ];

  const TYPEFACES: { value: Typeface; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'editorial', label: 'Editorial' },
    { value: 'literary', label: 'Literary' },
    { value: 'technical', label: 'Technical' },
  ];

  const PALETTES: { value: Palette; label: string }[] = [
    { value: 'default', label: 'Default' },
    { value: 'teal', label: 'Teal' },
    { value: 'amber', label: 'Amber' },
    { value: 'forest', label: 'Forest' },
    { value: 'violet', label: 'Violet' },
  ];

  function docRoot(): HTMLElement | null {
    return document.getElementById('doc');
  }

  /** Applies to `:root`, persists, and — for the three controls that can
   *  reflow the document — preserves reading position across the reflow. */
  function commit(next: Settings, reflows: boolean): void {
    settings = next;
    const root = docRoot();

    const apply = () => applySettingsToRoot(document.documentElement, next, EDITION);
    if (reflows && root) preserveScrollAcrossReflow(root, apply);
    else apply();

    void writeSettings(next).catch(() => {
      // Best-effort: a failed write means the next launch falls back to
      // whatever settings.json last held (or defaults). The control itself
      // already reflects the change, which matters more than surfacing a
      // disk-write error over a preference.
    });
  }

  function setTheme(theme: Theme): void {
    commit({ ...settings, theme }, false);
  }

  function setDensity(density: Density): void {
    commit({ ...settings, density }, true);
  }

  function setMeasure(measure: number): void {
    commit({ ...settings, measure }, true);
  }

  // `true`: zooming reflows the document, so the scroll position has to be
  // re-anchored to its source line afterwards — the same reason the
  // column-width slider passes it.
  function setZoom(zoom: number): void {
    commit({ ...settings, zoom: clampZoom(zoom) }, true);
  }

  /**
   * Ctrl+wheel and Ctrl+plus/minus/0, the way every browser does it.
   *
   * Handled here rather than in `app.svelte` because this component owns the
   * settings state and the write path; routing it through the app shell would
   * mean a second place that can change a preference.
   *
   * `preventDefault` matters twice over: the webview has its own Ctrl+wheel
   * page zoom, and leaving it enabled would scale the chrome as well as the
   * document — the exact behaviour this release is removing.
   */
  function onZoomWheel(event: WheelEvent): void {
    if (!event.ctrlKey || event.deltaY === 0) return;
    event.preventDefault();
    setZoom(settings.zoom * (event.deltaY < 0 ? ZOOM_STEP : 1 / ZOOM_STEP));
  }

  function onZoomKey(event: KeyboardEvent): void {
    if (!event.ctrlKey || event.altKey || event.metaKey) return;
    // `=` is the unshifted key `+` sits on, and is what people press.
    if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      setZoom(settings.zoom * ZOOM_STEP);
    } else if (event.key === '-' || event.key === '_') {
      event.preventDefault();
      setZoom(settings.zoom / ZOOM_STEP);
    } else if (event.key === '0') {
      event.preventDefault();
      setZoom(DEFAULT_SETTINGS.zoom);
    }
  }

  function setAccentHue(accent_hue: number): void {
    commit({ ...settings, accent_hue, palette: 'default' }, false);
  }

  function setTypeface(typeface: Typeface): void {
    commit({ ...settings, typeface }, true);
  }

  function setPalette(palette: Palette): void {
    commit({ ...settings, palette }, false);
  }

  onMount(() => {
    // `passive: false` — a passive listener cannot preventDefault, and without
    // that the webview's own Ctrl+wheel page zoom fires as well and scales the
    // chrome along with the document.
    window.addEventListener('wheel', onZoomWheel, { passive: false });
    window.addEventListener('keydown', onZoomKey);

    readSettings()
      .then((s) => {
        settings = s;
        applySettingsToRoot(document.documentElement, s, EDITION);
      })
      .catch(() => {
        // No persisted settings yet (or the command isn't wired up), or the
        // read failed — the defaults already applied via the initial state
        // are a correct, visible fallback either way.
        applySettingsToRoot(document.documentElement, settings, EDITION);
      })
      .finally(() => {
        loaded = true;
      });

    return () => {
      window.removeEventListener('wheel', onZoomWheel);
      window.removeEventListener('keydown', onZoomKey);
    };
  });
</script>

<div class="settings">

  {#if open}
    <div id="settings-panel" class="panel" class:panel-loading={!loaded}>
      <fieldset>
        <legend>Theme</legend>
        <div class="segmented" role="radiogroup" aria-label="Theme">
          {#each THEMES as t (t.value)}
            <button
              type="button"
              class:active={settings.theme === t.value}
              aria-pressed={settings.theme === t.value}
              onclick={() => setTheme(t.value)}
            >
              {t.label}
            </button>
          {/each}
        </div>
      </fieldset>

      <fieldset>
        <legend>Density</legend>
        <div class="segmented" role="radiogroup" aria-label="Density">
          {#each DENSITIES as d (d.value)}
            <button
              type="button"
              class:active={settings.density === d.value}
              aria-pressed={settings.density === d.value}
              onclick={() => setDensity(d.value)}
            >
              {d.label}
            </button>
          {/each}
        </div>
      </fieldset>

      <fieldset>
        <legend>
          <label for="zoom-range">Zoom — {settings.zoom}%</label>
        </legend>
        <input
          id="zoom-range"
          type="range"
          min={ZOOM_MIN}
          max={ZOOM_MAX}
          step="10"
          value={settings.zoom}
          oninput={(e) => setZoom(Number(e.currentTarget.value))}
        />
      </fieldset>

      <fieldset>
        <legend>
          <label for="measure-range">Column width — {settings.measure}ch</label>
        </legend>
        <input
          id="measure-range"
          type="range"
          min={MEASURE_MIN}
          max={MEASURE_MAX}
          value={settings.measure}
          oninput={(e) => setMeasure(Number(e.currentTarget.value))}
        />
      </fieldset>

      {#if EDITION === 'full'}
        <fieldset>
          <legend>
            <label for="typeface-select">Typeface</label>
          </legend>
          <select
            id="typeface-select"
            value={settings.typeface}
            onchange={(e) => setTypeface(e.currentTarget.value as Typeface)}
          >
            {#each TYPEFACES as t (t.value)}
              <option value={t.value}>{t.label}</option>
            {/each}
          </select>
        </fieldset>

        <fieldset>
          <legend>Palette</legend>
          <div class="segmented" role="radiogroup" aria-label="Palette">
            {#each PALETTES as p (p.value)}
              <button
                type="button"
                class:active={settings.palette === p.value}
                aria-pressed={settings.palette === p.value}
                onclick={() => setPalette(p.value)}
              >
                {p.label}
              </button>
            {/each}
          </div>
        </fieldset>
      {/if}

      <fieldset>
        <legend>
          <label for="hue-range">Accent hue — {settings.accent_hue}&deg;</label>
        </legend>
        <input
          id="hue-range"
          type="range"
          min="0"
          max="360"
          value={settings.accent_hue}
          oninput={(e) => setAccentHue(Number(e.currentTarget.value))}
        />
      </fieldset>
    </div>
  {/if}
</div>

<style>
  .settings {
    block-size: 100%;
    overflow-y: auto;
    font-family: var(--font-ui);
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4);
  }

  .panel-loading {
    opacity: 0.6;
    pointer-events: none;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    border: none;
  }

  legend {
    padding: 0;
    color: var(--fg-muted);
    font-size: var(--text-small);
  }

  .segmented {
    display: flex;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .segmented button {
    flex: 1;
    padding: var(--space-1) var(--space-2);
    background: none;
    border: none;
    border-inline-start: 1px solid var(--border);
    color: var(--fg-muted);
    font-family: inherit;
    font-size: var(--text-small);
    cursor: pointer;
    transition: color var(--duration-fast) ease, background var(--duration-fast) ease;
  }

  .segmented button:first-child {
    border-inline-start: none;
  }

  .segmented button:hover {
    color: var(--fg);
    background: var(--bg-subtle);
  }

  .segmented button.active {
    color: var(--on-accent);
    background: var(--accent);
  }

  input[type='range'] {
    accent-color: var(--accent);
    inline-size: 100%;
  }

  select {
    padding: var(--space-1) var(--space-2);
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-family: inherit;
    font-size: var(--text-small);
  }

</style>
