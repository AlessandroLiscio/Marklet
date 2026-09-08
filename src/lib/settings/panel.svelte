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
  import { applySettingsToRoot, DEFAULT_SETTINGS, MEASURE_MAX, MEASURE_MIN, preserveScrollAcrossReflow } from './model';

  const EDITION: 'lite' | 'full' = __MARKLET_EDITION__;

  let open = $state(false);
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

  function setMotion(on: boolean): void {
    commit({ ...settings, motion: on ? 'on' : 'off' }, false);
  }

  function setMeasure(measure: number): void {
    commit({ ...settings, measure }, true);
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
  });
</script>

<div class="settings">
  <button
    type="button"
    class="toggle"
    aria-expanded={open}
    aria-controls="settings-panel"
    aria-label="Settings"
    onclick={() => (open = !open)}
  >
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="2.25" stroke="currentColor" stroke-width="1.5" />
      <path
        d="M8 1.5v1.6M8 12.9v1.6M14.5 8h-1.6M3.1 8H1.5M12.4 3.6l-1.13 1.13M4.73 11.27L3.6 12.4M12.4 12.4l-1.13-1.13M4.73 4.73L3.6 3.6"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
      />
    </svg>
  </button>

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

      <fieldset>
        <legend>
          <label for="motion-toggle">Motion</label>
        </legend>
        <label class="switch">
          <input
            id="motion-toggle"
            type="checkbox"
            checked={settings.motion === 'on'}
            onchange={(e) => setMotion(e.currentTarget.checked)}
          />
          <span>{settings.motion === 'on' ? 'On' : 'Off'}</span>
        </label>
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
          disabled={EDITION === 'full' && settings.palette !== 'default'}
          oninput={(e) => setAccentHue(Number(e.currentTarget.value))}
        />
      </fieldset>
    </div>
  {/if}
</div>

<style>
  .settings {
    position: fixed;
    inset-block-end: var(--space-4);
    inset-inline-end: var(--space-4);
    font-family: var(--font-ui);
    z-index: 10;
  }

  .toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    inline-size: 2.25rem;
    block-size: 2.25rem;
    padding: 0;
    background: color-mix(in oklab, var(--bg) 88%, transparent);
    color: var(--fg-muted);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    cursor: pointer;
    backdrop-filter: blur(6px);
    transition: color var(--duration-fast) ease, border-color var(--duration-fast) ease;
  }

  .toggle:hover {
    color: var(--fg);
    border-color: var(--fg-muted);
  }

  .panel {
    position: absolute;
    inset-block-end: calc(2.25rem + var(--space-2));
    inset-inline-end: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    inline-size: 18rem;
    max-block-size: 70vh;
    overflow-y: auto;
    padding: var(--space-4);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    transition: opacity var(--duration-fast) ease;
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

  .switch {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--fg);
    font-size: var(--text-small);
    cursor: pointer;
  }

  .switch input {
    accent-color: var(--accent);
  }
</style>
