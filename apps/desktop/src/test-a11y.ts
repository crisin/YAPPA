// The axe runner for UI tests. jsdom has no layout and no canvas, so the colour-contrast
// rule cannot run there and only logs noise — contrast is checked by eye on the tokens.
// Everything else (names, roles, labels, landmarks, keyboard reachability) runs.
import { configureAxe } from 'vitest-axe';

export const axe = configureAxe({
  rules: { 'color-contrast': { enabled: false } },
});
