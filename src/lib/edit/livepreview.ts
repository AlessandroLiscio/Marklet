/**
 * F2 — live preview. The CodeMirror half of the rules in `syntax.ts`.
 *
 * A `ViewPlugin` that rebuilds a `DecorationSet` whenever the document, the
 * viewport or the selection changes, hiding markdown syntax on every line the
 * cursor is not on. That is the entire behaviour, and it is worth being blunt
 * about how small it is, because the alternative — a WYSIWYG editor over a
 * rich-text model — costs a markdown serializer and loses the hand-aligned
 * table on the first save. Here the buffer never stops being markdown text;
 * the decorations are a lens over it, and closing the lens changes nothing.
 *
 * Only the **visible ranges** are ever decorated. A 3 MB document has hundreds
 * of thousands of syntax nodes and CodeMirror only renders the few hundred
 * lines on screen, so walking the whole tree would be the one thing capable of
 * making this feature feel slow.
 *
 * Nothing in this file imports CodeMirror. The modules arrive as the `cm`
 * argument, already loaded by `cm.ts` — see that file for why the indirection
 * is not optional.
 */
import type { CodeMirror, DecorationSet, EditorView, Extension, Range } from './cm';
import {
  LINE_CLASSES,
  UNDECORATED,
  actionFor,
  appliesWhileRevealed,
  revealedLines,
} from './syntax';

/**
 * Builds the extension. Called once per editor, after CodeMirror has loaded.
 *
 * A factory rather than a module-level constant because `Decoration`,
 * `ViewPlugin` and `WidgetType` are all *values* from a lazily-imported module
 * — there is nothing to reference at module scope, and pretending otherwise is
 * how a lazy chunk quietly becomes an eager one.
 */
export function livePreview(cm: CodeMirror): Extension {
  const { Decoration, ViewPlugin, WidgetType } = cm.view;
  const { syntaxTree } = cm.language;

  const Widget = WidgetType;

  /** A bullet, a checkbox — a short stand-in for punctuation that was there. */
  class Stand extends Widget {
    constructor(
      readonly text: string,
      readonly cls: string,
    ) {
      super();
    }

    override eq(other: Stand): boolean {
      return other.text === this.text && other.cls === this.cls;
    }

    override toDOM(): HTMLElement {
      const span = document.createElement('span');
      span.className = this.cls;
      span.textContent = this.text;
      return span;
    }

    /** Clicking a rendered bullet should put the cursor there, not be eaten. */
    override ignoreEvent(): boolean {
      return false;
    }
  }

  /**
   * The block class for one line: the nearest enclosing container that has
   * one. Resolved from the syntax tree rather than by matching the line's text,
   * so a `#` inside a fenced code block is not mistaken for a heading.
   */
  function lineClass(view: EditorView, pos: number): string | null {
    let node = syntaxTree(view.state).resolveInner(pos, 1);
    for (;;) {
      const cls = LINE_CLASSES[node.name];
      if (cls !== undefined) return cls;
      const parent = node.parent;
      if (!parent) return null;
      node = parent;
    }
  }

  function build(view: EditorView): DecorationSet {
    const { state } = view;
    const deco: Range[] = [];

    const revealed = revealedLines(
      state.selection.ranges.map((range) => ({ from: range.from, to: range.to })),
      (pos) => state.doc.lineAt(pos).number,
    );

    for (const { from, to } of view.visibleRanges) {
      const firstLine = state.doc.lineAt(from).number;
      const lastLine = state.doc.lineAt(to).number;

      for (let n = firstLine; n <= lastLine; n += 1) {
        const line = state.doc.line(n);
        const cls = lineClass(view, line.from);
        if (cls !== null) deco.push(Decoration.line({ class: cls }).range(line.from));
      }

      syntaxTree(state).iterate({
        from,
        to,
        enter: (node) => {
          // A table (or a reference-link block) is left exactly as typed, and
          // that includes everything inside it — so the walk stops here rather
          // than descending and deciding node by node.
          if (UNDECORATED.has(node.name)) return false;

          const needsText = node.name === 'TaskMarker' || node.name === 'ListMark';
          const text = needsText ? state.doc.sliceString(node.from, node.to) : '';
          const action = actionFor(node.name, node.node.parent?.name ?? null, text);
          if (action === null) return undefined;

          const onRevealedLine = revealed.has(state.doc.lineAt(node.from).number);
          if (onRevealedLine && !appliesWhileRevealed(action)) return undefined;

          if (action.kind === 'mark') {
            if (node.to > node.from) {
              deco.push(Decoration.mark({ class: action.cls }).range(node.from, node.to));
            }
            return undefined;
          }

          if (action.kind === 'replace') {
            deco.push(
              Decoration.replace({ widget: new Stand(action.text, action.cls) }).range(
                node.from,
                node.to,
              ),
            );
            return undefined;
          }

          // 'hide'. A heading's `#` and a blockquote's `>` are each followed by
          // a space that exists only to separate them from the text; leaving it
          // behind indents every heading in the document by one character.
          let end = node.to;
          if (
            (node.name === 'HeaderMark' || node.name === 'QuoteMark') &&
            state.doc.sliceString(end, end + 1) === ' '
          ) {
            end += 1;
          }
          if (end > node.from) deco.push(Decoration.replace({}).range(node.from, end));
          return undefined;
        },
      });
    }

    // `true` sorts: line decorations and the inline ones are collected in two
    // separate passes and CodeMirror requires one ordered set.
    return Decoration.set(deco, true);
  }

  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        this.decorations = build(view);
      }

      update(update: { docChanged: boolean; viewportChanged: boolean; selectionSet: boolean; view: EditorView }): void {
        // `selectionSet` is the one that makes this *live*: moving the cursor
        // onto a line has to reveal that line's markdown on the same frame,
        // with no edit involved.
        if (update.docChanged || update.viewportChanged || update.selectionSet) {
          this.decorations = build(update.view);
        }
      }
    },
    { decorations: (plugin) => plugin.decorations },
  );
}
