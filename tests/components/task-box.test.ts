import { describe, expect, it } from 'vitest';
import { taskBoxOf } from '../../src/lib/tasks';

/** The markup the render core emits for a task list, tight and loose. */
function items(html: string): HTMLElement[] {
  const root = document.createElement('div');
  root.innerHTML = html;
  return Array.from(root.querySelectorAll<HTMLElement>('li'));
}

describe('taskBoxOf', () => {
  it('finds the box of a tight item', () => {
    const [li] = items('<ul><li data-l="3"><input class="task" type="checkbox" /> one</li></ul>');
    expect(taskBoxOf(li!)).not.toBeNull();
  });

  it('finds the box of a loose item, where it sits inside the <p>', () => {
    const [a, b] = items(
      '<ul><li data-l="3"><p data-l="3"><input class="task" type="checkbox" /> one</p></li>' +
        '<li data-l="5"><p data-l="5"><input class="task" type="checkbox" /> two</p></li></ul>',
    );
    expect(taskBoxOf(a!)).not.toBeNull();
    expect(taskBoxOf(b!)).not.toBeNull();
  });

  it("does not hand a parent the nested item's box", () => {
    const [parent] = items(
      '<ul><li data-l="1">plain<ul><li data-l="2"><input class="task" type="checkbox" /> kid</li></ul></li></ul>',
    );
    expect(taskBoxOf(parent!)).toBeNull();
  });
});
