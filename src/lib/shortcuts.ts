/**
 * Every key Marklet answers to, in one list.
 *
 * **Why this file exists.** An audit of the window found 34 capabilities with
 * no on-screen control of any kind — live-preview editing, image paste, the
 * snippets, half the tab commands. The application was roughly a third
 * discoverable. Adding 34 buttons would have been a worse window than 34
 * secrets, so the answer is one sheet that lists them, and this is what the
 * sheet reads.
 *
 * **Why it is data and not markup.** A help sheet written as prose drifts from
 * the keymap the first time a binding moves, and the drift is silent: a sheet
 * that names a key nothing binds is a lie on screen, which is worse than
 * saying nothing. `scripts/check-shortcuts.mjs` reads this file and fails the
 * build when a listed key is no longer compared for in the file that is
 * supposed to bind it, or when a key compared for in `src/` is missing here.
 *
 * That gate proves the key is still *handled where this file says it is*. It
 * cannot prove the handler still does what {@link Shortcut.what} claims — no
 * cheap check can, and pretending otherwise would be the same false confidence
 * this file was written to remove.
 *
 * Pure data. No DOM, no imports, no cost to anything that does not render it.
 */

export interface Shortcut {
  /** As a reader would write it. `+` joins a chord, `,` separates alternatives. */
  keys: string;
  /** What it does, in the second person, as a sentence fragment. */
  what: string;
  /**
   * Which file compares for this key.
   *
   * `null` means the binding is CodeMirror's own default keymap rather than
   * ours — we install the keymap, not the individual key, so there is nothing
   * in `src/` for the gate to find and it skips these rather than pretending.
   */
  where: string | null;
  /** Full edition only. Omitted means both. */
  full?: true;
}

export interface Group {
  title: string;
  /** One line under the heading, where the group needs it. Most do not. */
  blurb?: string;
  items: Shortcut[];
  /** Full edition only. Omitted means both. */
  full?: true;
  /**
   * Render the snippet table's own keys instead of `items`.
   *
   * The trigger words are `SNIPPETS` in `edit/snippets.ts`. Listing them again
   * here would be exactly the drift this file exists to prevent, so the group
   * is a flag that says "read that table" rather than a copy of it. The sheet
   * imports it; this file stays free of imports.
   */
  snippets?: true;
}

const APP = 'src/app.svelte';
const EDIT = 'src/lib/edit/index.ts';
const SESSION = 'src/lib/edit/session.ts';
const SETTINGS = 'src/lib/settings/panel.svelte';
const MERMAID = 'src/lib/rich/mermaid.ts';
const SHEET = 'src/lib/chrome/shortcuts.svelte';
const TREE = 'src/lib/sidebar/tree.svelte';

export const GROUPS: Group[] = [
  {
    title: 'Opening and reading',
    items: [
      { keys: 'Ctrl+O', what: 'open a file', where: APP },
      { keys: 'Ctrl+Shift+O', what: 'open a folder', where: APP },
      { keys: 'Ctrl + wheel', what: 'zoom the document', where: null },
      { keys: 'Ctrl+=, Ctrl+-', what: 'zoom in, zoom out', where: SETTINGS },
      { keys: 'Ctrl+0', what: 'back to 100%', where: SETTINGS },
      { keys: 'Double-click', what: 'open a note from the explorer — a single click only selects', where: null },
    ],
  },
  {
    title: 'The explorer tree',
    blurb: 'The tree is one tab stop: Tab enters it, the arrow keys move inside it. Moving only selects; Enter opens.',
    items: [
      { keys: '↑, ↓', what: 'previous or next row', where: TREE },
      { keys: '→', what: 'open a closed folder, or step into an open one', where: TREE },
      { keys: '←', what: 'close an open folder, or step out to its parent', where: TREE },
      { keys: 'Home, End', what: 'first or last row', where: TREE },
      { keys: 'Enter', what: 'open the note, or open and close the folder', where: TREE },
      { keys: 'Space', what: 'select the row, and open or close a folder', where: TREE },
    ],
  },
  {
    title: 'Tabs',
    blurb: 'The strip appears from the second tab onwards, so with one document open the keys are the only way.',
    items: [
      { keys: 'Ctrl+W', what: 'close this tab', where: APP },
      { keys: 'Ctrl+Shift+T', what: 'reopen the last closed tab, ten deep', where: APP },
      { keys: 'Ctrl+Tab', what: 'next tab', where: APP },
      { keys: 'Ctrl+Shift+Tab', what: 'previous tab', where: APP },
      { keys: 'Middle-click', what: 'close the tab you clicked', where: null },
    ],
  },
  {
    title: 'Editing',
    blurb:
      'Nothing is written to the file until you save: Ctrl+S or the Save button, which lights up while there are unsaved changes. In the split view the preview catches up on each save. F2 and F3 also swap straight to each other, keeping the cursor and the undo history.',
    items: [
      { keys: 'F2', what: 'live preview — edit with the markdown hidden on every line but the one you are on', where: EDIT },
      { keys: 'F3', what: 'split — source on the left, preview on the right, scrolling together', where: EDIT },
      { keys: 'Ctrl+S', what: 'save your changes to the file', where: EDIT },
      { keys: 'Esc', what: 'back to reading — asks first if there are unsaved changes', where: EDIT },
      { keys: 'F4', what: 'open this file in your own editor, at the cursor', where: EDIT },
      { keys: 'Ctrl+E', what: 'the same, but only while reading — the editor keeps this key for itself', where: EDIT },
      { keys: 'Ctrl+V', what: 'paste an image from the clipboard into assets/ and link it here', where: null },
      { keys: 'Tab', what: 'expand a snippet, or indent', where: SESSION },
    ],
  },
  {
    title: 'Snippets',
    blurb: 'Type one of these and press Tab. It cuts both ways: a prose word that happens to be on the list expands too, so press Ctrl+Z if it catches you.',
    snippets: true,
    items: [],
  },
  {
    title: 'In the editor',
    blurb: 'CodeMirror’s own keys, which Marklet installs whole rather than one at a time.',
    items: [
      { keys: 'Ctrl+Z, Ctrl+Y', what: 'undo, redo', where: null },
      { keys: 'Alt+↑, Alt+↓', what: 'move this line up or down', where: null },
      { keys: 'Shift+Alt+↑, Shift+Alt+↓', what: 'copy this line up or down', where: null },
      { keys: 'Ctrl+Shift+K', what: 'delete this line', where: null },
      { keys: 'Ctrl+/', what: 'comment out the selection', where: null },
      { keys: 'Ctrl+click', what: 'add another cursor', where: null },
      { keys: 'Alt + drag', what: 'select a rectangle', where: null },
      { keys: 'Shift+Tab', what: 'outdent', where: null },
      { keys: 'Ctrl+[, Ctrl+]', what: 'indent less, indent more', where: null },
      { keys: 'Alt+L', what: 'select the whole line', where: null },
      { keys: 'Ctrl+Enter', what: 'open a blank line below, wherever the cursor is', where: null },
      { keys: 'Ctrl+M', what: 'let Tab leave the editor instead of indenting', where: null },
    ],
  },
  {
    title: 'The split divider',
    items: [
      { keys: 'Drag', what: 'move the seam between the columns', where: null },
      { keys: 'Double-click', what: 'put it back in the middle', where: null },
      { keys: '←, →', what: 'move it by keyboard once it has focus — hold Shift for bigger steps', where: APP },
      { keys: 'Home', what: 'back to the middle', where: APP },
      { keys: 'Enter', what: 'back to the middle', where: APP },
    ],
  },
  {
    title: 'Diagrams',
    blurb: 'Click a Mermaid diagram to open it over the page.',
    full: true,
    items: [
      { keys: 'Wheel', what: 'zoom about the pointer', where: null, full: true },
      { keys: 'Drag', what: 'pan', where: null, full: true },
      { keys: '+, -', what: 'zoom in, zoom out', where: MERMAID, full: true },
      { keys: '0', what: 'back to fit', where: MERMAID, full: true },
      { keys: 'Double-click', what: 'back to fit', where: null, full: true },
      { keys: 'Esc', what: 'close the viewer', where: MERMAID, full: true },
      { keys: 'Click outside', what: 'close the viewer', where: null, full: true },
    ],
  },
  {
    title: 'Exporting',
    blurb: 'Both ask where to save. Both are also on the activity bar, under Export.',
    items: [
      { keys: 'Ctrl+P', what: 'export a PDF of what is on screen', where: APP },
      { keys: 'Ctrl+Shift+S', what: 'export one standalone HTML file', where: APP },
    ],
  },
  {
    title: 'No key needed',
    blurb: 'Marklet does these on its own. They are here because nothing else on screen says so.',
    items: [
      { keys: '—', what: 'the document reloads when the file changes on disk, keeping your place', where: null },
      { keys: '—', what: 'each tab remembers the line you left it on, until the window closes', where: null },
      { keys: '—', what: 'opening a note from Explorer while Marklet is running adds a tab here', where: null },
    ],
  },
  {
    title: 'This sheet',
    items: [
      { keys: 'F1', what: 'open and close it', where: APP },
      { keys: 'Esc', what: 'close it', where: SHEET },
    ],
  },
];
