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

/** Opens `svg` in a fullscreen overlay with wheel-zoom and drag-pan. */
function openViewer(svg: string): void {
  const overlay = document.createElement('div');
  overlay.className = 'mermaid-viewer';
  overlay.innerHTML =
    `<div class="mermaid-viewer-stage">${svg}</div>` +
    `<button class="mermaid-viewer-close" type="button" aria-label="Close">✕</button>`;
  document.body.appendChild(overlay);

  const stage = overlay.querySelector<HTMLElement>('.mermaid-viewer-stage')!;
  const closeBtn = overlay.querySelector<HTMLElement>('.mermaid-viewer-close')!;
  let scale = 1;
  let x = 0;
  let y = 0;
  let dragging = false;
  let lastX = 0;
  let lastY = 0;

  const apply = () => {
    stage.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
  };

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const prev = scale;
    scale = Math.min(8, Math.max(0.25, scale * (e.deltaY < 0 ? 1.1 : 1 / 1.1)));
    const rect = overlay.getBoundingClientRect();
    const cx = e.clientX - rect.left - rect.width / 2;
    const cy = e.clientY - rect.top - rect.height / 2;
    x -= cx * (scale / prev - 1);
    y -= cy * (scale / prev - 1);
    apply();
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
    if (e.key === 'Escape') close();
  };
  const onOverlayClick = (e: MouseEvent) => {
    if (e.target === overlay) close();
  };

  function close(): void {
    overlay.removeEventListener('wheel', onWheel);
    stage.removeEventListener('pointerdown', onPointerDown);
    stage.removeEventListener('pointermove', onPointerMove);
    stage.removeEventListener('pointerup', onPointerUp);
    overlay.removeEventListener('click', onOverlayClick);
    document.removeEventListener('keydown', onKeydown);
    overlay.remove();
  }

  overlay.addEventListener('wheel', onWheel, { passive: false });
  stage.addEventListener('pointerdown', onPointerDown);
  stage.addEventListener('pointermove', onPointerMove);
  stage.addEventListener('pointerup', onPointerUp);
  overlay.addEventListener('click', onOverlayClick);
  closeBtn.addEventListener('click', close);
  document.addEventListener('keydown', onKeydown);
}
