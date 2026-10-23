// App versions are `major.minor.patch` (Cargo). Anything else — a dev label, a typo — is
// "unknown", and an unknown version is never blocked: better a call than a false lockout.

/** Negative, zero or positive like a sort comparator; `null` if either side is unknown. */
export function compareVersions(a: string, b: string): number | null {
  const parse = (v: string) => {
    const match = /^v?(\d+)\.(\d+)\.(\d+)/.exec(v.trim());
    return match ? match.slice(1, 4).map(Number) : null;
  };
  const left = parse(a);
  const right = parse(b);
  if (!left || !right) return null;
  for (let i = 0; i < 3; i++) {
    if (left[i] !== right[i]) return left[i] - right[i];
  }
  return 0;
}

/** Is this app version below the server's minimum or on its kill list? */
export function isBlocked(version: string, minVersion: string, killed: readonly string[]): boolean {
  if (killed.includes(version)) return true;
  const order = compareVersions(version, minVersion);
  return order !== null && order < 0;
}
