import { describe, expect, it } from 'vitest';
import {
  applySettingsToRoot,
  clampMeasure,
  DEFAULT_SETTINGS,
  MEASURE_MAX,
  MEASURE_MIN,
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
  it('sets no theme/density/motion attribute for the defaults ("system"/"normal"/"on")', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, DEFAULT_SETTINGS, 'lite');
    expect(root.attributes.has('data-theme')).toBe(false);
    expect(root.attributes.has('data-density')).toBe(false);
    expect(root.attributes.has('data-motion')).toBe(false);
  });

  it('sets data-theme for an explicit choice', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, theme: 'dark' }, 'lite');
    expect(root.attributes.get('data-theme')).toBe('dark');
  });

  it('sets data-motion="off" only when motion is off', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, motion: 'off' }, 'lite');
    expect(root.attributes.get('data-motion')).toBe('off');
  });

  it('always writes a clamped --measure custom property', () => {
    const root = fakeRoot();
    applySettingsToRoot(root, { ...DEFAULT_SETTINGS, measure: 500 }, 'lite');
    expect(root.properties.get('--measure')).toBe('100ch');
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
