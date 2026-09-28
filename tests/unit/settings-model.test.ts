import { describe, expect, it } from 'vitest';
import {
  applySettingsToRoot,
  clampMeasure,
  clampZoom,
  DEFAULT_SETTINGS,
  MEASURE_MAX,
  MEASURE_MIN,
  ZOOM_MAX,
  ZOOM_MIN,
} from '../../src/lib/settings/model';
import type { StyleTarget } from '../../src/lib/settings/model';
import type { Settings } from '../../src/lib/ipc';

/** A minimal stand-in for `document.documentElement` — `vitest.config.ts`
 *  runs under Node with no DOM, so `applySettingsToRoot` is exercised
 *  against a plain object satisfying the same narrow interface it declares
 *  for exactly this reason. */
function fakeRoot(): StyleTarget & {
  attributes: Map<string, string>;
  properties: Map<string, string>;
} {
  const attributes = new Map<string, string>();
  const properties = new Map<string, string>();
  return {
    attributes,
    properties,
    setAttribute: (name, value) => attributes.set(name, value),
    removeAttribute: (name) => attributes.delete(name),
    style: {
      setProperty: (name, value) => properties.set(name, value),
      removeProperty: (name) => properties.delete(name),
    },
  };
}

describe('clampMeasure', () => {
  it('passes through an in-range value', () => {
    expect(clampMeasure(68)).toBe(68);
  });

  it('clamps below the minimum', () => {
    expect(clampMeasure(10)).toBe(MEASURE_MIN);
  });

  it('clamps above the maximum', () => {
    expect(clampMeasure(500)).toBe(MEASURE_MAX);
  });

  it('rounds a fractional value', () => {
    expect(clampMeasure(68.6)).toBe(69);
  });
});

describe('applySettingsToRoot', () => {
  it('sets no theme or density attribute for the defaults ("system"/"normal")', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, DEFAULT_SETTINGS, 'lite');
    expect(root.attributes.has('data-theme')).toBe(false);
    expect(root.attributes.has('data-density')).toBe(false);
    // `data-motion` is gone entirely: the toggle that set it gated four
    // transitions under 200ms and could not be told apart from nothing. The
    // OS `prefers-reduced-motion` preference is what remains, and nothing in
    // this function touches it.
    expect(root.attributes.has('data-motion')).toBe(false);
  });

  it('sets data-theme for an explicit choice', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, theme: 'dark' }, 'lite');
    expect(root.attributes.get('data-theme')).toBe('dark');
  });

  it('always writes a clamped --measure custom property', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, measure: 500 }, 'lite');
    expect(root.properties.get('--measure')).toBe('100ch');
  });

  it('writes zoom as a clamped multiplier, not a root font size', () => {
    // A custom property read by `#doc { zoom: … }` — the document column and
    // nothing else. It was the root font size for one release, which scaled
    // the sidebar and the settings panel along with it.
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, zoom: 1000 }, 'lite');
    expect(root.properties.get('--doc-zoom')).toBe(String(ZOOM_MAX / 100));
    expect(root.properties.has('font-size')).toBe(false);

    const small = fakeRoot();
    applySettingsToRoot(small, { ...DEFAULT_SETTINGS, zoom: 1 }, 'lite');
    expect(small.properties.get('--doc-zoom')).toBe(String(ZOOM_MIN / 100));
  });

  it('clamps a zoom from outside the slider', () => {
    // Ctrl+wheel multiplies without bound, so the clamp is the only thing
    // between a fast scroll and an unreadable document.
    expect(clampZoom(0)).toBe(ZOOM_MIN);
    expect(clampZoom(100)).toBe(100);
    expect(clampZoom(1e9)).toBe(ZOOM_MAX);
    expect(clampZoom(102.6)).toBe(103);
  });

  it('defaults match what tokens.css and store.rs declare', () => {
    // Three constants live in three places with no build-time link: this file,
    // `src/styles/tokens.css`, and `store::Settings::default`. Each side has a
    // test naming the numbers so a change to one fails the others.
    expect(DEFAULT_SETTINGS.accent_hue).toBe(70); // Material Amber 700, in oklch
    expect(DEFAULT_SETTINGS.measure).toBe(100);
    expect(DEFAULT_SETTINGS.zoom).toBe(100);
  });

  it('never sets data-typeface or data-palette in the lite edition', () => {
    const root = fakeRoot();
    const settings: Settings = { ...DEFAULT_SETTINGS, typeface: 'literary', palette: 'teal' };
    applySettingsToRoot(root, settings, 'lite');
    expect(root.attributes.has('data-typeface')).toBe(false);
    expect(root.attributes.has('data-palette')).toBe(false);
    // The custom hue still applies in lite — palette is a full-only control.
    expect(root.properties.get('--accent-hue')).toBe(String(DEFAULT_SETTINGS.accent_hue));
  });

  it('sets data-typeface in the full edition', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, typeface: 'technical' }, 'full');
    expect(root.attributes.get('data-typeface')).toBe('technical');
  });

  it('a non-default palette sets data-palette and clears the inline accent-hue', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, palette: 'forest' }, 'full');
    expect(root.attributes.get('data-palette')).toBe('forest');
    // Cleared, not merely left stale: an inline style always wins over the
    // palette's own [data-palette] CSS rule, so leaving it set would
    // silently override the palette the user just picked.
    expect(root.properties.has('--accent-hue')).toBe(false);
  });

  it('switching back to the default palette restores the inline accent-hue', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, palette: 'forest' }, 'full');
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, palette: 'default', accent_hue: 99 }, 'full');
    expect(root.attributes.has('data-palette')).toBe(false);
    expect(root.properties.get('--accent-hue')).toBe('99');
  });
});
