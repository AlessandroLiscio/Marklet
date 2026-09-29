# Marklet demo vault

The folder to open with **Open folder…** when you want to exercise everything Marklet
does with a *vault* rather than with a single file. [[demo]] is the document itself — the
one with a section per feature.

Small on purpose. Four files is enough to show a folder tree, a search with more than one
match, a wiki-link that resolves into a subfolder, and a note with more than one thing
linking to it; a hundred would show the same behaviour and hide it in scrolling.

## What is in here

| File | Why it exists |
|---|---|
| `demo.md` | the test document, eighteen sections |
| `README.md` | this note, so `[[README]]` in `demo.md` has something to resolve to |
| `docs/architecture.md` | a note one folder down, for `[[docs/architecture]]` |
| `assets/marklet-icon.png` | the image `demo.md` displays |

## What to look at here specifically

Open `demo.md` and look at the **Linked from** panel at the bottom of the explorer: this
note and `docs/architecture.md` both point at it, so both should be listed, each with the
line the link is on. Ctrl+click a row to open it in a second window.

Then open this note. Its own **Linked from** should list `demo.md`, which links to
[[README]] in section 12 — the other direction of the same edge.

See also [[docs/architecture]].
