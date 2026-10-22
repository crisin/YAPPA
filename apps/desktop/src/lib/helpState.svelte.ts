// Where the help is: open or not, which page, which heading. Shared by the footer button,
// the help itself and the ⓘ tooltips ("Mehr im Glossar"), so any of them can open a page.
import { pageById, pagesIn, GROUPS } from './docs';

class Help {
  open = $state(false);
  page = $state<string | null>(null);
  anchor = $state<string | null>(null);

  /** Open the help at a page (default: the first one) and optionally scroll to a heading. */
  show(page?: string, anchor?: string) {
    this.page = pageById(page ?? null)?.id ?? pagesIn(GROUPS[0].id)[0]?.id ?? null;
    this.anchor = anchor ?? null;
    this.open = true;
  }

  toggle() {
    if (this.open) this.open = false;
    else this.show(this.page ?? undefined);
  }
}

export const help = new Help();
