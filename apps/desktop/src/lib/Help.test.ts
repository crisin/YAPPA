import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import Help from './Help.svelte';
import { glossary, pages } from './docs';
import { help } from './helpState.svelte';

beforeEach(() => {
  help.open = false;
  help.page = null;
  help.anchor = null;
});

describe('Help', () => {
  it('starts with the tester guide and lists both groups', () => {
    render(Help);
    expect(screen.getByRole('heading', { level: 1, name: 'Mittesten' })).toBeInTheDocument();
    const nav = screen.getByRole('navigation', { name: 'Hilfe-Seiten' });
    expect(within(nav).getByText('Hilfe')).toBeInTheDocument();
    expect(within(nav).getByText('Projekt')).toBeInTheDocument();
    expect(within(nav).getByRole('button', { name: 'Mittesten' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(screen.getByText('docs/mittesten.md')).toBeInTheDocument();
  });

  it('switches pages from the navigation', async () => {
    render(Help);
    await fireEvent.click(screen.getByRole('button', { name: 'Status' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Tasks' })).toBeInTheDocument();
    expect(help.page).toBe('tasks');
  });

  it('follows links between pages', async () => {
    help.show('hilfe/so-funktioniert-es');
    render(Help);
    const article = screen.getByRole('article');
    await fireEvent.click(within(article).getByRole('link', { name: 'Glossar' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Glossar' })).toBeInTheDocument();
    expect(help.page).toBe('hilfe/glossar');
  });

  it('scrolls to the heading of a glossary term', async () => {
    const scrolled: Element[] = [];
    Element.prototype.scrollIntoView = function () {
      scrolled.push(this);
    };
    help.show('hilfe/glossar', 'jitter');
    render(Help);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(scrolled[0]).toHaveAttribute('id', 'jitter');
  });

  it('searches titles and text', async () => {
    render(Help);
    await fireEvent.input(screen.getByRole('searchbox'), { target: { value: 'jitterbuffer' } });
    const nav = screen.getByRole('navigation', { name: 'Hilfe-Seiten' });
    expect(within(nav).getByText(/Treffer/)).toBeInTheDocument();
    expect(within(nav).getByRole('button', { name: 'Glossar' })).toBeInTheDocument();
    expect(within(nav).queryByRole('button', { name: 'Changelog' })).not.toBeInTheDocument();
  });

  it('carries every page the repo lists, with a title', () => {
    for (const page of pages) {
      expect(page.title).not.toBe('');
      expect(page.html).toContain('<h1');
    }
    expect(pages.map((page) => page.id)).toContain('changelog');
    expect(Object.keys(glossary).length).toBeGreaterThan(10);
  });
});
