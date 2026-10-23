import '@testing-library/jest-dom/vitest';
import { expect } from 'vitest';
import * as axeMatchers from 'vitest-axe/matchers';

// `expect(await axe(container)).toHaveNoViolations()` — accessibility is tested, not
// promised. Registered here by hand: the package's own `extend-expect` entry targets an
// older vitest and does not reach this one's `expect`.
expect.extend(axeMatchers);

// axe probes a canvas even with the colour-contrast rule off; jsdom has none and would log
// "Not implemented" on every run. A null context is what a browser without canvas returns.
HTMLCanvasElement.prototype.getContext = (() =>
  null) as typeof HTMLCanvasElement.prototype.getContext;

// jsdom has no layout: Bits UI's command list measures itself and scrolls the selection
// into view. Neither matters for behaviour, both must exist.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
Element.prototype.scrollIntoView ??= function () {};
