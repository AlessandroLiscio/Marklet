# Architecture

A note one folder down, so `[[docs/architecture]]` in [[demo]] has a real target and the
folder tree has something to expand.

Wiki-links are resolved **by name**, not by relative path: `[[demo]]` from here finds
`demo.md` in the folder above without a `../`, which is what makes a vault a graph rather
than a directory. The subfolder is still visible in the path the **Linked from** panel
shows, so you can tell where a referrer lives.

The real architecture document is [`docs/architecture.md`](https://github.com/AlessandroLiscio/Marklet/blob/main/docs/architecture.md)
in the repository, not this file. This one exists to be linked to.
