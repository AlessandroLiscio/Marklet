/**
 * The one place CodeMirror 6 enters the bundle, and it enters lazily.
 *
 * **Every reference below is `import('@codemirror/…')`, never `import … from`,
 * including in type position.** `npm run check:imports` is a flat text scan,
 * not a reachability graph: a static import here would fail the build even
 * though this module is only ever reached through another `import()`, and it
 * would be right to — see `src/lib/rich/hljs.ts`, which has the same shape for
 * the same reason. The types are recovered with `typeof import(…)`, which is
 * erased at compile time and matches nothing the scanner looks for.
 *
 * `vite.config.ts`'s `manualChunks` groups every `node_modules/@codemirror` and
 * `node_modules/@lezer` module into the single `codemirror` output chunk, so
 * the five imports below are one network request and one cache entry, not
 * five. The budget for it is ~180 KB xz
 * (`.claude/skills/size-budget/SKILL.md`), charged only to a session that
 * actually presses F2, F3 or Ctrl+E. A read-only session downloads none of it.
 */

export type StateModule = typeof import('@codemirror/state');
export type ViewModule = typeof import('@codemirror/view');
export type CommandsModule = typeof import('@codemirror/commands');
export type LanguageModule = typeof import('@codemirror/language');
export type MarkdownModule = typeof import('@codemirror/lang-markdown');

/** Everything the editor layer uses, loaded together. */
export interface CodeMirror {
  state: StateModule;
  view: ViewModule;
  commands: CommandsModule;
  language: LanguageModule;
  markdown: MarkdownModule;
}

// Referenced as types, not via `InstanceType<...>`: both classes declare a
// private constructor, which `abstract new (...args: any) => any` rejects.
export type EditorView = import('@codemirror/view').EditorView;
export type EditorState = import('@codemirror/state').EditorState;
export type Extension = import('@codemirror/state').Extension;
export type DecorationSet = import('@codemirror/view').DecorationSet;
export type Range = import('@codemirror/state').Range<
  import('@codemirror/view').Decoration
>;

let pending: Promise<CodeMirror> | null = null;

/**
 * Loads CodeMirror once and hands the same modules back to every later caller.
 *
 * Memoized on the *promise*, not on the result: F2 and F3 can be pressed
 * within the same frame, and two concurrent callers must not each start their
 * own load and then race to install two editors.
 */
export function loadCodeMirror(): Promise<CodeMirror> {
  pending ??= Promise.all([
    import('@codemirror/state'),
    import('@codemirror/view'),
    import('@codemirror/commands'),
    import('@codemirror/language'),
    import('@codemirror/lang-markdown'),
  ]).then(([state, view, commands, language, markdown]) => ({
    state,
    view,
    commands,
    language,
    markdown,
  }));
  return pending;
}

/** Test seam: forgets the memoized load. Not called by the app. */
export function resetCodeMirrorForTests(): void {
  pending = null;
}
