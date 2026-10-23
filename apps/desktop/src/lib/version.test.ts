import { describe, expect, it } from 'vitest';
import { compareVersions, isBlocked } from './version';

describe('versions', () => {
  it('compares numerically, not as text', () => {
    expect(compareVersions('0.10.0', '0.9.9')).toBeGreaterThan(0);
    expect(compareVersions('1.2.3', '1.2.3')).toBe(0);
    expect(compareVersions('v0.1.0', '0.2.0')).toBeLessThan(0);
  });

  it('calls dev labels unknown and never blocks them', () => {
    expect(compareVersions('browser', '0.1.0')).toBeNull();
    expect(isBlocked('browser', '99.0.0', [])).toBe(false);
  });

  it('blocks below the minimum and on the kill list', () => {
    expect(isBlocked('0.1.0', '99.0.0', [])).toBe(true);
    expect(isBlocked('0.4.2', '0.4.0', ['0.4.2'])).toBe(true);
    expect(isBlocked('0.4.2', '0.4.0', [])).toBe(false);
  });
});
