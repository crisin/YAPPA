// Times in the admin screens: relative for „when“, short for log lines.

const relative = new Intl.RelativeTimeFormat('de', { numeric: 'auto' });

/** „gerade eben“, „vor 5 Minuten“, „vor 3 Tagen“, „in 7 Tagen“. */
export function since(rfc3339: string, now = Date.now()): string {
  const seconds = (new Date(rfc3339).getTime() - now) / 1000;
  if (Number.isNaN(seconds)) return '–';
  if (Math.abs(seconds) < 60) return 'gerade eben';
  const steps: [Intl.RelativeTimeFormatUnit, number][] = [
    ['minute', 60],
    ['hour', 60 * 60],
    ['day', 60 * 60 * 24],
  ];
  let unit: Intl.RelativeTimeFormatUnit = 'minute';
  let size = 60;
  for (const [u, s] of steps) {
    if (Math.abs(seconds) >= s) {
      unit = u;
      size = s;
    }
  }
  return relative.format(Math.round(seconds / size), unit);
}

/** „23.10. 21:04“ for the event log. */
export function stamp(rfc3339: string): string {
  const date = new Date(rfc3339);
  if (Number.isNaN(date.getTime())) return rfc3339;
  const two = (n: number) => String(n).padStart(2, '0');
  return `${two(date.getDate())}.${two(date.getMonth() + 1)}. ${two(date.getHours())}:${two(date.getMinutes())}`;
}
