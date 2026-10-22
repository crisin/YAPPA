// The in-app help: rendered from the repo by `cargo xtask docs` (docs/, TASKS.md,
// CHANGELOG.md -> docs/content.json). The UI only reads; the markdown files are the source.
import content from './docs/content.json';

export interface Page {
  id: string;
  group: string;
  title: string;
  source: string;
  html: string;
  text: string;
}

/** One `##` entry of docs/hilfe/glossar.md; `short` is the tooltip text. */
export interface Entry {
  term: string;
  short: string;
  html: string;
}

export const pages: Page[] = content.pages;
export const glossary: Record<string, Entry> = content.glossary;

export const GROUPS: { id: string; label: string }[] = [
  { id: 'hilfe', label: 'Hilfe' },
  { id: 'projekt', label: 'Projekt' },
];

export const GLOSSARY_PAGE = 'hilfe/glossar';

export function pagesIn(group: string): Page[] {
  return pages.filter((page) => page.group === group);
}

export function pageById(id: string | null): Page | undefined {
  return pages.find((page) => page.id === id);
}

/** Pages whose title or text contains every word of the query (case-insensitive). */
export function search(query: string): Page[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  return pages.filter((page) => {
    const haystack = `${page.title} ${page.text}`.toLowerCase();
    return words.every((word) => haystack.includes(word));
  });
}
