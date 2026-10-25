// Updates as the UI sees them (ADR-006, D1): the last check's answer and whether one is
// running. The check, the download and the install happen in the shell; this only asks
// through the Api door and keeps the answer for the banner and the Updates page.
import type { Api } from './api';
import type { ApiError } from './types/ApiError';
import type { UpdateStatus } from './types/UpdateStatus';

export class Updater {
  /** `null` until the first check. */
  status = $state<UpdateStatus | null>(null);
  checking = $state(false);
  checkedAt = $state<Date | null>(null);

  readonly #api: Api;

  constructor(api: Api) {
    this.#api = api;
  }

  /** The downloaded update, if the last check found one. */
  get ready(): { version: string; notes: string } | null {
    return this.status?.kind === 'ready' ? this.status : null;
  }

  /** Asks the shell; a second call while one runs does nothing. Never throws. */
  async check(): Promise<void> {
    if (this.checking) return;
    this.checking = true;
    try {
      this.status = await this.#api.checkUpdate();
    } catch (reason) {
      const error = reason as Partial<ApiError> | undefined;
      this.status = { kind: 'failed', reason: error?.message ?? String(reason) };
    } finally {
      this.checking = false;
      this.checkedAt = new Date();
    }
  }

  /** Installs the downloaded update; the app restarts, so nothing comes back. */
  install(): Promise<void> {
    return this.#api.installUpdate();
  }
}
