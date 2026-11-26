// @vitest-environment happy-dom
/**
 * Enter in a bullet list must never turn a tight list loose (#147).
 *
 * One blank line between two items makes CommonMark render the *whole* list
 * loose: every item gains a paragraph's margin.
 */
import { describe, expect, it } from 'vitest';
import { createSession } from '../../src/lib/edit/session';

async function press(source: string, key: string): Promise<string> {
  const host = document.createElement('div');
  document.body.append(host);
  const session = await createSession({
    host,
    path: '/vault/note.md',
    source,
    decorated: false,
    readSource: async () => source,
  });
  // The parser works on a time budget; on a loaded machine it may not be done.
  // The state only picks the finished tree up with the next transaction.
  session.cm.language.ensureSyntaxTree(session.view.state, source.length, 5000);
  session.view.dispatch({ selection: { anchor: source.length } });
  session.cm.view.runScopeHandlers(session.view, new KeyboardEvent('keydown', { key }), 'editor');
  const text = session.text();
  session.destroy();
  host.remove();
  return text;
}

describe('Enter in a bullet list', () => {
  it('on an empty second item leaves the list instead of adding a blank line', async () => {
    expect(await press('- a\n- ', 'Enter')).toBe('- a\n');
  });

  it('continues a tight list without a blank line', async () => {
    expect(await press('- a\n- b', 'Enter')).toBe('- a\n- b\n- ');
  });

  it('keeps a list that is already loose as it is', async () => {
    expect(await press('- a\n\n- b', 'Enter')).toBe('- a\n\n- b\n\n- ');
  });

  it('keeps Backspace removing the list markup', async () => {
    expect(await press('- a\n- ', 'Backspace')).toBe('- a\n  ');
  });
});
