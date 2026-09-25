/**
 * What the clipboard half of Ctrl+V has to get right in the frontend.
 *
 * Naming the file and writing it are `save_pasted_image` in Rust — a filename
 * the webview chose would be a filename the webview could choose badly, and
 * the slug and the numbering are tested next to the directory listing they
 * depend on, in `ipc::paste_and_export_tests`. Likewise F4: the editor table
 * and its argv construction moved to `src-tauri/src/editor.rs`, so that the
 * webview names a line and a column and never a program. See that module's
 * doc comment.
 *
 * What is left here is the part that is *wrong until somebody checks it*: the
 * markdown escaping of a destination with a space in it, and which clipboard
 * flavour to take when a paste offers several.
 */
import { describe, expect, it } from 'vitest';

import { extensionFor, imageMarkdown, pickImage } from '../../src/lib/edit/paste';

describe('pasted image naming', () => {
  it('wraps a destination containing a space in angle brackets', () => {
    expect(imageMarkdown('assets/a.png')).toBe('![](assets/a.png)');
    expect(imageMarkdown('assets/a b.png', 'Alt')).toBe('![Alt](<assets/a b.png>)');
  });

  it('takes only image types it can name', () => {
    expect(extensionFor('image/png')).toBe('png');
    expect(extensionFor('IMAGE/JPEG')).toBe('jpg');
    expect(extensionFor('text/plain')).toBeNull();
  });

  it('prefers the lossless format when the clipboard offers several', () => {
    const picked = pickImage([{ type: 'image/jpeg' }, { type: 'image/png' }]);
    expect(picked).toEqual({ type: 'image/png', ext: 'png' });
    expect(pickImage([{ type: 'text/html' }])).toBeNull();
  });
});
