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

Open this note and look at the **Links** panel at the bottom of the explorer. It lists
what *this* file points at, in the order the links appear:

- [[demo]] and [[docs/architecture]], as notes, with the line each one is on,
- [the repository](https://github.com/AlessandroLiscio/Marklet), as a web link — a globe
  rather than a page, and clicking it opens your browser rather than replacing the
  document,
- [[A note nobody has written]], struck through and inert, because nothing in this folder
  answers to that name.

Clicking a **row** scrolls this document to the line the link is written on. Following a
link is the small button on the right — Ctrl+click it for a second window, and a web link
goes to your browser either way. Two destinations, two targets: a stray click on a list of
links should not replace what you are reading.
