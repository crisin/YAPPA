import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import Info from './Info.svelte';
import { glossary } from './docs';
import { help } from './helpState.svelte';

beforeEach(() => {
  help.open = false;
});

// Every `term="…"` in the components must be a glossary entry — a typo would otherwise
// just make the ⓘ disappear.
const sources = import.meta.glob<string>(['./**/*.svelte', '../App.svelte'], {
  query: '?raw',
  import: 'default',
  eager: true,
});

describe('Info', () => {
  it('shows the one-sentence explanation on click and opens the glossary', async () => {
    render(Info, { term: 'jitter' });
    const mark = screen.getByRole('button', { name: 'Erklärung: Jitter' });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    await fireEvent.click(mark);
    expect(screen.getByRole('tooltip')).toHaveTextContent(glossary.jitter.short);
    await fireEvent.click(screen.getByRole('button', { name: 'Mehr im Glossar' }));
    expect(help.open).toBe(true);
    expect(help.page).toBe('hilfe/glossar');
    expect(help.anchor).toBe('jitter');
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('renders nothing for an unknown term', () => {
    const { container } = render(Info, { term: 'gibt-es-nicht' });
    expect(container.querySelector('button')).toBeNull();
  });

  it('only uses terms the glossary defines', () => {
    const used = new Set<string>();
    for (const source of Object.values(sources)) {
      for (const match of source.matchAll(/term[:=]\s*['"]([a-z0-9-]+)['"]/g)) used.add(match[1]);
    }
    expect(used.size).toBeGreaterThan(5);
    for (const term of used) expect(glossary, term).toHaveProperty(term);
  });
});
