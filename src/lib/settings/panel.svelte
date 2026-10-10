<script lang="ts">
  /**
   * The settings panel — inside the main window in both editions this wave
   * (per `docs/editions.md`, full's *dedicated window* is a later phase, not
   * this one). Every control here writes through to `store.rs` via the
   * `read_settings` / `write_settings` commands in `ipc.ts`.
   *
   * **A floating card in the bottom-right corner, not a docked panel.** It was
   * docked into the activity bar for one release and asked back out. The
   * reasoning holds up: the other two panels change what you are looking at
   * and want the document to move aside for them, while settings is a thing
   * you open, change and dismiss — it should sit over the document rather
   * than push it.
   *
   * Self-contained: it owns its own open state and resolves `#doc` and
   * `:root` itself, so `app.svelte` mounts it with no props.
   *
   * Typeface and palette are full-edition-only controls, eliminated from the
   * lite bundle at compile time by `if (__MARKLET_EDITION__ === 'full')` —
   * see `docs/editions.md` and `.claude/skills/size-budget/SKILL.md`.
   */
  import { onMount } from 'svelte';
  import { checkUpdate, installUpdate, readSettings, writeSettings } from '../ipc';
  import type { UpdateInfo } from '../ipc';
  import type { Density, Palette, Settings, Theme, Typeface } from '../ipc';
  import { applySettingsToRoot, clampZoom, DEFAULT_SETTINGS, PALETTE_HUES, MEASURE_MAX, MEASURE_MIN, preserveScrollAcrossReflow, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP } from './model';

  const EDITION: 'lite' | 'full' = __MARKLET_EDITION__;

  // `marklet --settings` opens it with the window rather than a moment after:
  // the flag is on `window` before the first frame, set by the boot script in
  // `lib.rs`, so this is a plain initializer and not an effect.
  let open = $state(
    typeof window !== 'undefined' && window.__MARKLET_SETTINGS__ === true
  );
  let settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  let loaded = $state(false);

  const THEMES: { value: Theme; label: string }[] = [
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ];

  const DENSITIES: { value: Density; label: string }[] = [
    { value: 'compact', label: 'Compact' },
    { value: 'normal', label: 'Normal' },
    { value: 'spacious', label: 'Spacious' },
  ];

  const TYPEFACES: { value: Typeface; label: string }[] = [
    { value: 'technical', label: 'Technical' },
    { value: 'system', label: 'System' },
    { value: 'editorial', label: 'Editorial' },
    { value: 'literary', label: 'Literary' },
  ];

  const PALETTES: { value: Palette; label: string }[] = [
    { value: 'default', label: 'Default' },
    { value: 'teal', label: 'Teal' },
    { value: 'amber', label: 'Amber' },
    { value: 'forest', label: 'Forest' },
    { value: 'violet', label: 'Violet' },
  ];

  /** Applies to `:root`, persists, and — for the three controls that can
   *  reflow the document — preserves reading position across the reflow. */
  function commit(next: Settings, reflows: boolean): void {
    settings = next;

    const apply = () => applySettingsToRoot(document.documentElement, next, EDITION);
    if (reflows) preserveScrollAcrossReflow(apply);
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

  /**
   * A palette is a hue. Picking one sets the slider to it, so the two controls
   * can never disagree about what the accent is.
   */
  function setPalette(palette: Palette): void {
    commit({ ...settings, palette, accent_hue: PALETTE_HUES[palette] }, false);
  }

  /** The hue actually in use: a named palette's own, else the slider's. */
  const hue = $derived(
    settings.palette !== 'default' ? PALETTE_HUES[settings.palette] : settings.accent_hue
  );

  /** The chip to light: the chosen palette, or none for a hue between them. */
  function chipOn(p: Palette): boolean {
    return settings.palette !== 'default' ? settings.palette === p : hue === PALETTE_HUES[p];
  }

  // Updates — full edition only; the branches are dead code in lite and the
  // bundler drops them. Never checked at launch: the idle timer below waits
  // until the reader has been looking at the window for a while, and the
  // button asks on demand.
  let update = $state<UpdateInfo | null>(null);
  let updateStatus = $state('');
  let updateBusy = $state(false);

  async function lookForUpdate(quiet: boolean): Promise<void> {
    updateBusy = true;
    if (!quiet) updateStatus = 'Checking…';
    try {
      update = await checkUpdate();
      if (!quiet) updateStatus = update ? '' : 'Marklet is up to date.';
    } catch (err) {
      // A quiet check that fails (offline, rate limit) says nothing at all.
      if (!quiet) updateStatus = `Could not check: ${(err as { message?: string })?.message ?? err}`;
    } finally {
      updateBusy = false;
    }
  }

  async function installNow(): Promise<void> {
    updateBusy = true;
    updateStatus = 'Downloading and verifying…';
    try {
      await installUpdate();
    } catch (err) {
      updateStatus = `Update failed: ${(err as { message?: string })?.message ?? err}`;
      updateBusy = false;
    }
  }

  onMount(() => {
    const idle =
      EDITION === 'full' ? window.setTimeout(() => void lookForUpdate(true), 120_000) : 0;
    // `passive: false` — a passive listener cannot preventDefault, and without
    // that the webview's own Ctrl+wheel page zoom fires as well and scales the
    // chrome along with the document.
    window.addEventListener('wheel', onZoomWheel, { passive: false });
    window.addEventListener('keydown', onZoomKey);

    readSettings()
      .then((read) => {
        // `system` is gone from the panel. A settings file that still says it
        // is read as whatever the OS prefers right now, and stays that until
        // the reader picks one.
        const s: Settings =
          read.theme === 'system'
            ? {
                ...read,
                theme: window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
              }
            : read;
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
      window.clearTimeout(idle);
      window.removeEventListener('wheel', onZoomWheel);
      window.removeEventListener('keydown', onZoomKey);
    };
  });
</script>

<div class="settings">
  <button
    type="button"
    class="toggle"
    class:on={open}
    aria-expanded={open}
    aria-controls="settings-panel"
    aria-label="Settings"
    title="Settings"
    onclick={() => (open = !open)}
  >
    {#if update}<span class="dot" aria-hidden="true"></span>{/if}
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
            <label for="typeface-select">Font</label>
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
      {/if}

      <fieldset>
        <legend>Palette</legend>
        {#if EDITION === 'full'}
          <!-- A grid, not the segmented strip the other two use: five labels
               do not fit across a 260px panel, and forcing them to produced a
               horizontal scrollbar across the whole settings column. -->
          <div class="chips" role="radiogroup" aria-label="Palette">
            {#each PALETTES as p (p.value)}
              <button
                type="button"
                class:active={chipOn(p.value)}
                aria-pressed={chipOn(p.value)}
                onclick={() => setPalette(p.value)}
              >
                {p.label}
              </button>
            {/each}
          </div>
        {/if}
        <!-- The fine adjustment under the presets, not a section of its own.
             Dragging it leaves every chip dark: a hue between two palettes is
             nobody's preset. -->
        <input
          id="hue-range"
          class="hue"
          type="range"
          min="0"
          max="360"
          aria-label="Accent hue"
          value={hue}
          oninput={(e) => setAccentHue(Number(e.currentTarget.value))}
        />
      </fieldset>

      {#if EDITION === 'full'}
        <fieldset>
          <legend>Updates</legend>
          {#if update}
            <p class="update-note">Marklet {update.version} is available.</p>
            <button type="button" class="update-btn" disabled={updateBusy} onclick={installNow}>
              Install and restart
            </button>
          {:else}
            <button
              type="button"
              class="update-btn"
              disabled={updateBusy}
              onclick={() => lookForUpdate(false)}
            >
              Check for updates
            </button>
          {/if}
          {#if updateStatus}<p class="update-note" role="status">{updateStatus}</p>{/if}
        </fieldset>
      {/if}
    </div>
  {/if}
</div>

<style>
  /* Clear of the status line in the very corner, which is why this is not the
     `--space-4` every other floating thing uses. The status line is one
     `--text-small` line plus `--space-1` top and bottom plus its border, so
     `--space-8` alone left it touching at normal density. */
  .settings {
    position: fixed;
    inset-block-end: calc(var(--space-8) + var(--space-3));
    inset-inline-end: var(--space-4);
    font-family: var(--font-ui);
    z-index: 30;
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

  /* Open reads as pressed by fill, the same rule the split toggle follows. */
  .toggle.on {
    color: var(--on-accent);
    background: var(--accent);
    border-color: var(--accent);
  }

  .toggle {
    position: relative;
  }

  /* An update is waiting. A shape and a position, not colour alone. */
  .dot {
    position: absolute;
    inset-block-start: 4px;
    inset-inline-end: 4px;
    inline-size: 8px;
    block-size: 8px;
    border-radius: 50%;
    background: var(--accent);
    border: 1px solid var(--bg);
  }

  .update-btn {
    padding: var(--space-1) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg);
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .update-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .update-note {
    margin: var(--space-1) 0 0;
    font-size: var(--text-small);
    color: var(--fg-muted);
  }

  .toggle:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
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
    /* Nothing inside is allowed to widen the card into a horizontal
       scrollbar — controls wrap or truncate instead. */
    overflow-x: hidden;
    padding: var(--space-4);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: 0 8px 24px oklch(0% 0 0 / 20%);
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
    flex: 1 1 0;
    /* `min-inline-size: 0` is what lets a flex item be narrower than its text.
       Without it the widest label sets the strip's floor and the panel grows a
       scrollbar rather than the label truncating. */
    min-inline-size: 0;
    padding: var(--space-1) var(--space-2);
    background: none;
    border: none;
    border-inline-start: 1px solid var(--border);
    color: var(--fg-muted);
    font-family: inherit;
    font-size: var(--text-small);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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

  .hue {
    margin-block-start: var(--space-3);
  }

  .chips {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-1);
  }

  .chips button {
    min-inline-size: 0;
    padding: var(--space-1);
    background: none;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--fg-muted);
    font-family: inherit;
    font-size: var(--text-small);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    transition: color var(--duration-fast) ease, background var(--duration-fast) ease;
  }

  .chips button:hover {
    color: var(--fg);
    background: var(--bg-subtle);
  }

  .chips button.active {
    color: var(--on-accent);
    background: var(--accent);
    border-color: var(--accent);
  }

  .chips button:focus-visible,
  .segmented button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
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
