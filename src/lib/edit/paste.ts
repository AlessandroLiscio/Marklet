/**
 * Ctrl+V of an image: where the file goes, what it is called, and what gets
 * typed into the document.
 *
 * Reading the clipboard and writing the markdown happen here; **choosing the
 * filename and writing the bytes do not.** Both of those are
 * `save_pasted_image` in `src-tauri/src/ipc.rs`, because the frontend has no
 * filesystem permission and because a name the webview picked would be a name
 * the webview could choose to be `../../autorun`. The slug and the numbering
 * therefore live in Rust, next to the directory listing they depend on, and
 * this module simply inserts whatever relative path comes back. See
 * `.claude/skills/tauri-ipc/SKILL.md`.
 *
 * The name matters more than it looks. A pasted screenshot that lands as
 * `image1.png` is unfindable six months later in a vault with four hundred of
 * them, and a name derived from the document is the difference between a
 * folder you can read and a folder you delete in frustration — which is why
 * `ipc::document_slug` exists at all rather than a counter.
 */

/** Which image formats the clipboard is worth reading, and their extensions. */
export const IMAGE_TYPES: Record<string, string> = {
  'image/png': 'png',
  'image/jpeg': 'jpg',
  'image/gif': 'gif',
  'image/webp': 'webp',
  'image/svg+xml': 'svg',
  'image/avif': 'avif',
};

/**
 * The markdown to insert for a saved image.
 *
 * A space is a legal character in a filename and an illegal one in a bare
 * markdown destination, so a path containing one is wrapped in angle brackets
 * — the CommonMark spelling, which `pulldown-cmark` parses and every other
 * tool does too. Percent-encoding would also work and would make the path
 * unreadable in the source, which is the file the user is actually editing.
 */
export function imageMarkdown(relativePath: string, alt = ''): string {
  const destination = /[\s()]/.test(relativePath) ? `<${relativePath}>` : relativePath;
  return `![${alt}](${destination})`;
}

/** The extension for a clipboard MIME type, or `null` if it is not an image we take. */
export function extensionFor(mime: string): string | null {
  return IMAGE_TYPES[mime.toLowerCase()] ?? null;
}

/**
 * The first image item on a clipboard, or `null`.
 *
 * Order follows {@link IMAGE_TYPES} rather than the clipboard's own order:
 * pasting from a browser commonly offers the same picture as both PNG and
 * JPEG, and taking the lossless one is free.
 */
export function pickImage(items: readonly { type: string }[]): { type: string; ext: string } | null {
  for (const mime of Object.keys(IMAGE_TYPES)) {
    const found = items.find((item) => item.type.toLowerCase() === mime);
    if (found) return { type: mime, ext: IMAGE_TYPES[mime] as string };
  }
  return null;
}
