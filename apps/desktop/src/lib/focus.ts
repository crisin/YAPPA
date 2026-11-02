// Where the keyboard lands after the shell moved something (ADR-004, review 2026-10-23):
// a module switch puts it into the new module, an opened inspector takes it, and closing
// returns it. No DOM is ever remembered across a module switch — the module's sidebar and
// stage are built anew, so a landing is found again each time.

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), ' +
  'textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Puts the keyboard into `root`: on what is current there (the open section, my channel,
 * the checked view), else on the first thing it can reach, else on `root` itself. Returns
 * what got the focus, `null` when there was nothing.
 */
export function focusInto(root: HTMLElement | null | undefined): HTMLElement | null {
  if (!root) return null;
  const current = root.querySelector<HTMLElement>('[aria-current], input:checked');
  const target =
    (current?.matches(FOCUSABLE) ? current : null) ?? root.querySelector<HTMLElement>(FOCUSABLE);
  if (target) {
    target.focus();
    return target;
  }
  if (!root.hasAttribute('tabindex')) root.tabIndex = -1;
  root.focus();
  return root;
}

/** `focus()` on an element that is still on the page; false when it is gone. */
export function refocus(element: Element | null | undefined): boolean {
  if (!(element instanceof HTMLElement) || !element.isConnected || element === document.body)
    return false;
  element.focus();
  return document.activeElement === element;
}
