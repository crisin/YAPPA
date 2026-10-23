// Hotkey names from the platform hook (crates/platform) as people say them.

const SPECIAL: Record<string, string> = {
  CapsLock: 'Feststelltaste',
  Space: 'Leertaste',
  Tab: 'Tab',
  ShiftLeft: 'Umschalt links',
  ShiftRight: 'Umschalt rechts',
  ControlLeft: 'Strg links',
  ControlRight: 'Strg rechts',
  Alt: 'Alt',
  AltGr: 'Alt Gr',
  BackQuote: 'Zirkumflex (^)',
  MouseMiddle: 'Mittlere Maustaste',
};

export function keyLabel(name: string | null | undefined): string {
  if (!name) return 'keine';
  if (SPECIAL[name]) return SPECIAL[name];
  let m = /^Key([A-Z])$/.exec(name);
  if (m) return m[1];
  m = /^Num(\d)$/.exec(name);
  if (m) return m[1];
  m = /^Kp(\d)$/.exec(name);
  if (m) return `Ziffernblock ${m[1]}`;
  m = /^Mouse(\d+)$/.exec(name);
  if (m) return `Maustaste ${m[1]}`;
  m = /^Unknown\((\d+)\)$/.exec(name);
  if (m) return `Taste ${m[1]}`;
  return name;
}
