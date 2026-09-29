// @vitest-environment happy-dom
/**
 * The Links panel's model: what a document points at, read back out of the
 * rendered document rather than out of the vault index.
 *
 * The index is the wrong source for this even though it holds the wiki-links:
 * it has no opinion about `https://`, and "what does this note link to" means
 * both kinds. The rendered document already holds every link, already
 * resolved, already inside a `data-l` block — so this is one pass over the
 * DOM, and it cannot disagree with what is on screen.
 */
import { describe, expect, it } from 'vitest';

import { collectLinks } from '../../src/lib/sidebar/links';

function render(html: string): HTMLElement {
  const root = document.createElement('article');
  root.innerHTML = html;
  return root;
}

describe('collectLinks', () => {
  /** Exactly what the renderer emits, copied from a real render of `demo/`. */
  const RESOLVED =
    '<a class="wikilink" href="marklet://localhost/docs/architecture.md" data-target="docs/architecture">Architecture</a>';

  it('reads a resolved wiki-link as its vault-relative path, not its name', () => {
    // The renderer resolved the name when it built the document and put the
    // answer in the href; `open_note` and `open_in_new_window` both take that
    // path, so reading `data-target` instead would mean resolving it twice —
    // and `data-target` has no extension, which `resolve_path` needs.
    const root = render(`<p data-l="7">${RESOLVED}</p>`);
    expect(collectLinks(root)).toEqual([
      { kind: 'note', label: 'Architecture', target: 'docs/architecture.md', line: 7 },
    ]);
  });

  it('reads both spellings of the asset scheme', () => {
    // `AssetRoot::url_prefix()` is `marklet://localhost/` on Linux and
    // `http://marklet.localhost/` on Windows, because WebView2 refuses a
    // genuinely custom scheme. Matching a fixed prefix is what broke this:
    // the string that was matched, `marklet://vault/`, is neither — it is what
    // an arbitrary *test* resolver returns in `render/wikilink.rs`, read as if
    // it were the format. Every resolved link was struck through.
    for (const href of [
      'marklet://localhost/docs/architecture.md',
      'http://marklet.localhost/docs/architecture.md',
    ]) {
      const root = render(`<p data-l="7"><a class="wikilink" href="${href}">A</a></p>`);
      expect(collectLinks(root)[0], href).toMatchObject({
        kind: 'note',
        target: 'docs/architecture.md',
      });
    }
  });

  it('treats an href with no path as unresolved rather than as a note', () => {
    const root = render('<p data-l="1"><a class="wikilink" href="marklet://localhost/">X</a></p>');
    expect(collectLinks(root)[0]?.kind).toBe('unresolved');
  });

  it('percent-decodes the path out of the href', () => {
    // `index.rs` percent-encodes the path into the URL, so a note with a space
    // in its name arrives as `%20` — and `open_note` takes the real path.
    const root = render(
      '<p data-l="5"><a class="wikilink" href="marklet://localhost/my%20notes/a%20b.md">A B</a></p>',
    );
    expect(collectLinks(root)[0]?.target).toBe('my notes/a b.md');
  });

  it('reads an unresolved wiki-link as the name that answered to nothing', () => {
    const root = render(
      '<p data-l="3"><a class="wikilink unresolved" data-target="Missing Note">Missing Note</a></p>',
    );
    expect(collectLinks(root)).toEqual([
      { kind: 'unresolved', label: 'Missing Note', target: 'Missing Note', line: 3 },
    ]);
  });

  it('includes http and https links, which the vault index knows nothing about', () => {
    const root = render(
      '<p data-l="2"><a href="https://example.com/a">Example</a> <a href="http://example.org">http://example.org</a></p>',
    );
    expect(collectLinks(root).map((l) => [l.kind, l.target])).toEqual([
      ['web', 'https://example.com/a'],
      ['web', 'http://example.org'],
    ]);
  });

  it('leaves out anchors, mailto and relative file links', () => {
    // Nothing in the application opens the last two, and an in-page anchor is
    // not a link to somewhere else. A row that does nothing when clicked is
    // worse than a row that is not there.
    const root = render(
      '<p data-l="1">' +
        '<a href="#section-2">Jump</a>' +
        '<a href="mailto:someone@example.com">Mail</a>' +
        '<a href="other.md">Other</a>' +
        '<a href="../up.md">Up</a>' +
        '</p>',
    );
    expect(collectLinks(root)).toEqual([]);
  });

  it('keeps every occurrence, in document order, each with its own line', () => {
    // Not collapsed: "every link in this document" is the question, and three
    // links to one note are three places to go, like three search hits.
    const root = render(
      '<p data-l="4"><a class="wikilink" href="marklet://vault/a.md">A</a></p>' +
        '<p data-l="9"><a href="https://example.com">Web</a></p>' +
        '<p data-l="12"><a class="wikilink" href="marklet://vault/a.md">A again</a></p>',
    );
    expect(collectLinks(root).map((l) => [l.target, l.line])).toEqual([
      ['a.md', 4],
      ['https://example.com', 9],
      ['a.md', 12],
    ]);
  });

  it('takes the line from the nearest enclosing block, however deep the link is', () => {
    const root = render(
      '<ul data-l="20"><li><em><a href="https://example.com">Deep</a></em></li></ul>',
    );
    expect(collectLinks(root)[0]?.line).toBe(20);
  });

  it('answers line 0 when there is no data-l to read', () => {
    const root = render('<p><a href="https://example.com">Loose</a></p>');
    expect(collectLinks(root)[0]?.line).toBe(0);
  });

  it('falls back to the target when the link has no text', () => {
    const root = render('<p data-l="1"><a href="https://example.com/x"></a></p>');
    expect(collectLinks(root)[0]?.label).toBe('https://example.com/x');
  });
});
