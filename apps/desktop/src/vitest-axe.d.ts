// Types for the axe matchers registered in test-setup.ts (the package's own declaration
// targets an older vitest with a global `Vi` namespace).
import type { AxeMatchers } from 'vitest-axe';

declare module 'vitest' {
  interface Assertion<T = unknown> extends AxeMatchers {}
  interface AsymmetricMatchersContaining extends AxeMatchers {}
}
