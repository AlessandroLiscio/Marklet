/**
 * Mermaid, lazy, full-edition only.
 *
 * Rust emits a placeholder and stops (`.claude/skills/render-pipeline/SKILL.md`):
 *
 *   <div class="mermaid" data-src="graph TD; A--&gt;B"></div>
 *
 * `renderMermaidIn` is called once, only when `index.ts`'s sniff finds at
 * least one such node. `vite.config.ts` aliases the bare `mermaid` package to
 * `./mermaid-stub.ts` when `MARKLET_EDITION=lite`, so in lite this module's
 * `import('mermaid')` resolves to that stub instead of the real ~750 KB
 * package, and the real chunk is never emitted at all.
 *
 * **API surface kept in step with `mermaid-stub.ts` on purpose**: this file
 * calls exactly two methods on the imported module — `.initialize(config)`
 * and `.render(id, source)` — because those are the only two the stub
 * implements. A stub that silently lacks a method the caller uses turns a
 * missing feature into a runtime crash, so nothing else from mermaid's much
 * larger API (`.parse`, `.run`, `.registerExternalDiagrams`, ...) is used
 * here. Both the real package and the stub expose only a `default` export
 * (no named `initialize`/`render`) — checked against
 * `node_modules/mermaid/dist/mermaid.core.mjs`'s own `export` statement —
 * so this module imports the default and calls through it, never a named
 * import, which would build fine against the stub and break at runtime
 * against the real package.
 *
 * `unsafe-eval`: **not needed.** `grep -rIl "new Function(\|eval(" node_modules/mermaid/`
 * (mermaid 11.17.2, as pinned) returns nothing — the installed package
 * contains no dynamic code generation. `securityLevel: 'strict'` is set
 * below anyway, independent of that: it is mermaid's own sanitization of
 * diagram *content* (disables `click` javascript: hrefs, runs labels through
 * DOMPurify), which matters because diagram source comes from the same
 * trust level as the rest of an opened markdown file, not a CSP concern.
 *
 * The fullscreen zoom/pan viewer below is hand-written (~60 lines of pointer
 * events plus a CSS transform) rather than a `panzoom` dependency — see
 * `.claude/skills/size-budget/SKILL.md`'s already-rejected list.
 */
import type { EnrichResult } from './types';
import './mermaid.css';

/** The subset of mermaid's API this module actually calls. */
interface MermaidApi {
  initialize(config: unknown): void;
  render(id: string, source: string): Promise<{ svg: string }>;
}

let apiPromise: Promise<MermaidApi> | null = null;
let counter = 0;

function loadMermaid(): Promise<MermaidApi> {
  apiPromise ??= import('mermaid').then((mod) => {
    const api = (mod as { default: MermaidApi }).default;
    api.initialize({ startOnLoad: false, securityLevel: 'strict', theme: 'neutral' });
    return api;
  });
  return apiPromise;
}

export async function renderMermaidIn(root: HTMLElement): Promise<EnrichResult> {
  const nodes = Array.from(root.querySelectorAll<HTMLElement>('.mermaid'));
  if (nodes.length === 0) return { rendered: 0 };

  const mermaid = await loadMermaid();
  let rendered = 0;
  for (const el of nodes) {
    const source = el.dataset['src'] ?? '';
    const id = `mermaid-diagram-${++counter}`;
    try {
      const { svg } = await mermaid.render(id, source);
      el.innerHTML = svg;
      el.classList.add('mermaid-rendered');
      el.tabIndex = 0;
      el.setAttribute('role', 'button');
      el.setAttribute('aria-label', 'Open diagram fullscreen');
      el.addEventListener('click', () => openViewer(svg));
      el.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          openViewer(svg);
        }
      });
      rendered++;
    } catch {
      // Malformed diagram source: show it as text rather than an empty box
      // or an error that takes the surrounding document down with it.
      el.textContent = source;
      el.classList.add('mermaid-error');
    }
  }
  return { rendered };
}

/**
 * Rasterizes `svg` through a `<canvas>`, or hands back its own bytes.
 *
 * No `html-to-image`: the only thing being exported is an `<svg>` element that
 * Mermaid just produced, and turning one of those into a PNG is three native
 * browser APIs in a row. A library for it would cost bytes to do the same work
 * less predictably — it is the webfont case that makes `html-to-image`
 * unreliable, and this markup carries its styling inline.
 *
 * Rendered at twice the intrinsic size: a diagram exported at CSS pixels looks
 * soft the moment it is put in a document or a slide.
 */
async function rasterize(svg: string, format: 'svg' | 'png'): Promise<Uint8Array> {
  if (format === 'svg') return new TextEncoder().encode(svg);

  const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
  try {
    const image = new Image();
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve();
      image.onerror = () => reject(new Error('the diagram could not be rasterized'));
      image.src = url;
    });

    const scale = 2;
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round((image.naturalWidth || 800) * scale));
    canvas.height = Math.max(1, Math.round((image.naturalHeight || 600) * scale));

    const context = canvas.getContext('2d');
    if (!context) throw new Error('no 2d context to draw the diagram on');
    context.drawImage(image, 0, 0, canvas.width, canvas.height);

    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
    if (!blob) throw new Error('the diagram produced no image data');
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    URL.revokeObjectURL(url);
  }
}

/**
 * Asks whoever owns the document to write the diagram beside it.
 *
 * A `CustomEvent` rather than a direct `invoke`: this module is reached only
 * through `enrich()` and has no idea which document is open, and importing the
 * IPC layer here to find out would put a second `invoke` call site outside
 * `src/lib/ipc.ts` — the one thing that file exists to prevent. `app.svelte`
 * listens and calls `save_pasted_image`, which already writes exactly this
 * kind of file next to the note and picks the name.
 */
async function saveDiagram(svg: string, format: 'svg' | 'png'): Promise<void> {
  try {
    const bytes = await rasterize(svg, format);
    document.dispatchEvent(
      new CustomEvent('marklet-save-asset', { detail: { bytes, ext: format } }),
    );
  } catch (error) {
    document.dispatchEvent(
      new CustomEvent('marklet-save-asset', {
        detail: { error: error instanceof Error ? error.message : 'the diagram could not be saved' },
      }),
    );
  }
}

/**
 * Opens `svg` in a fullscreen overlay.
 *
 * Zoom: the wheel, the `+` / `−` buttons, `+` / `-` on the keyboard, `0` or a
 * double-click to reset. More than one way in on purpose — wheel zoom was
 * reported dead in a build where dragging worked, so the buttons are the path
 * that does not depend on a wheel event arriving.
 */
function openViewer(svg: string): void {
  const overlay = document.createElement('div');
  overlay.className = 'mermaid-viewer';
  overlay.innerHTML =
    `<div class="mermaid-viewer-stage">${svg}</div>` +
    `<div class="mermaid-viewer-zoom">` +
    `<button class="mermaid-viewer-step" type="button" data-step="out" aria-label="Zoom out">\u2212</button>` +
    `<output class="mermaid-viewer-level">100%</output>` +
    `<button class="mermaid-viewer-step" type="button" data-step="in" aria-label="Zoom in">+</button>` +
    `<button class="mermaid-viewer-step" type="button" data-step="reset">Reset</button>` +
    `</div>` +
    `<div class="mermaid-viewer-tools">` +
    `<button class="mermaid-viewer-save" type="button" data-format="svg">Save SVG</button>` +
    `<button class="mermaid-viewer-save" type="button" data-format="png">Save PNG</button>` +
    `</div>` +
    `<button class="mermaid-viewer-close" type="button" aria-label="Close">✕</button>`;
  document.body.appendChild(overlay);

  const stage = overlay.querySelector<HTMLElement>('.mermaid-viewer-stage')!;
  const closeBtn = overlay.querySelector<HTMLElement>('.mermaid-viewer-close')!;

  for (const button of overlay.querySelectorAll<HTMLButtonElement>('.mermaid-viewer-save')) {
    button.addEventListener('click', (e) => {
      e.stopPropagation();
      void saveDiagram(svg, button.dataset['format'] === 'png' ? 'png' : 'svg');
    });
  }
  let scale = 1;
  let x = 0;
  let y = 0;
  let dragging = false;
  let lastX = 0;
  let lastY = 0;

  const level = overlay.querySelector<HTMLOutputElement>('.mermaid-viewer-level')!;

  const apply = () => {
    stage.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
    level.textContent = `${Math.round(scale * 100)}%`;
  };

  /**
   * Zooms by `factor`, keeping the point `(cx, cy)` — measured from the centre
   * of the overlay — under the cursor.
   *
   * Extracted from the wheel handler so the buttons and the keyboard drive the
   * exact same arithmetic. `zoom.test.ts` covers it directly; the event
   * plumbing around it cannot be unit-tested and, on the evidence, cannot be
   * relied on either (see `onWheel`).
   */
  function zoomBy(factor: number, cx = 0, cy = 0): void {
    const prev = scale;
    scale = Math.min(8, Math.max(0.25, scale * factor));
    if (scale === prev) return;
    x -= cx * (scale / prev - 1);
    y -= cy * (scale / prev - 1);
    apply();
  }

  /**
   * Wheel zoom, written defensively because it was reported not working at all
   * while dragging worked — which rules out the transform, the stage and the
   * maths, and leaves event delivery.
   *
   * Three things are handled that the first version assumed away:
   *
   *   - `deltaMode`. A wheel event reports in pixels, lines or pages, and only
   *     the sign was ever read. Reading the sign alone is fine; reading the
   *     magnitude without the mode is not, so only the sign is used here too,
   *     deliberately.
   *   - `ctrlKey`. A trackpad pinch arrives as a wheel event with `ctrlKey`
   *     set, which the browser otherwise turns into a page zoom.
   *   - the listener is attached in the CAPTURE phase on the overlay *and* on
   *     the stage. If something inside Mermaid's own SVG stops propagation —
   *     and its pan-zoom-aware diagram types do — a bubble-phase listener on
   *     the overlay never sees the event.
   *
   * The zoom controls exist so that none of this has to be right.
   */
  const onWheel = (e: WheelEvent) => {
    if (e.deltaY === 0) return;
    e.preventDefault();
    e.stopPropagation();
    const rect = overlay.getBoundingClientRect();
    zoomBy(
      e.deltaY < 0 ? 1.1 : 1 / 1.1,
      e.clientX - rect.left - rect.width / 2,
      e.clientY - rect.top - rect.height / 2,
    );
  };

  const onPointerDown = (e: PointerEvent) => {
    dragging = true;
    lastX = e.clientX;
    lastY = e.clientY;
    stage.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: PointerEvent) => {
    if (!dragging) return;
    x += e.clientX - lastX;
    y += e.clientY - lastY;
    lastX = e.clientX;
    lastY = e.clientY;
    apply();
  };
  const onPointerUp = (e: PointerEvent) => {
    dragging = false;
    stage.releasePointerCapture(e.pointerId);
  };
  const onKeydown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      close();
      return;
    }
    // `=` is the unshifted key `+` lives on, and is what people actually press.
    if (e.key === '+' || e.key === '=') {
      e.preventDefault();
      zoomBy(1.2);
    } else if (e.key === '-' || e.key === '_') {
      e.preventDefault();
      zoomBy(1 / 1.2);
    } else if (e.key === '0') {
      e.preventDefault();
      scale = 1;
      x = 0;
      y = 0;
      apply();
    }
  };
  const onOverlayClick = (e: MouseEvent) => {
    if (e.target === overlay) close();
  };

  function close(): void {
    overlay.removeEventListener('wheel', onWheel, { capture: true });
    stage.removeEventListener('wheel', onWheel, { capture: true });
    stage.removeEventListener('pointerdown', onPointerDown);
    stage.removeEventListener('pointermove', onPointerMove);
    stage.removeEventListener('pointerup', onPointerUp);
    overlay.removeEventListener('click', onOverlayClick);
    document.removeEventListener('keydown', onKeydown);
    overlay.remove();
  }

  for (const button of overlay.querySelectorAll<HTMLButtonElement>('.mermaid-viewer-step')) {
    button.addEventListener('click', (e) => {
      e.stopPropagation();
      const step = button.dataset['step'];
      if (step === 'in') zoomBy(1.2);
      else if (step === 'out') zoomBy(1 / 1.2);
      else {
        scale = 1;
        x = 0;
        y = 0;
        apply();
      }
    });
  }

  // Capture phase, and on both nodes: see `onWheel`.
  overlay.addEventListener('wheel', onWheel, { passive: false, capture: true });
  stage.addEventListener('wheel', onWheel, { passive: false, capture: true });
  // Double-click resets, which is the gesture people try first when a diagram
  // has ended up somewhere unhelpful.
  stage.addEventListener('dblclick', () => {
    scale = 1;
    x = 0;
    y = 0;
    apply();
  });
  apply();
  stage.addEventListener('pointerdown', onPointerDown);
  stage.addEventListener('pointermove', onPointerMove);
  stage.addEventListener('pointerup', onPointerUp);
  overlay.addEventListener('click', onOverlayClick);
  closeBtn.addEventListener('click', close);
  document.addEventListener('keydown', onKeydown);
}
