/**
 * Fragment queue management for streaming playback
 * Handles queuing, processing, and auto-advancement of audio fragments
 */

/** Fragment queued for streaming playback */
export interface QueuedFragment {
  audioBase64: string;
  index: number;
  total: number;
  text: string;
  captions?: import("#lib/models/captions").CaptionAlignment | null;
  /** Browser-companion session this clip belongs to, if one follows it */
  sessionId?: string | null;
}

export interface FragmentQueueHandlers {
  onFragmentPlay: (fragment: QueuedFragment) => Promise<void>;
  onQueueComplete: () => void;
}

/** How many already-played fragments are retained for cross-boundary rewind. */
const MAX_REWIND_FRAGMENTS = 3;

export class FragmentQueue {
  private _queue: QueuedFragment[] = [];
  /** Most recently played fragments, oldest first, for {@link previousFragment}. */
  private _played: QueuedFragment[] = [];
  /** All fragments received for the current reading, preserved for replay. */
  private _allFragments: QueuedFragment[] = [];
  private _isProcessing = false;
  private _waitingForNext = false;
  private _terminal = false;
  private _handlers: FragmentQueueHandlers;

  constructor(handlers: FragmentQueueHandlers) {
    this._handlers = handlers;
  }

  /**
   * Determine if more fragments are expected after the last played fragment.
   */
  private hasMoreFragmentsExpected(): boolean {
    const last = this._played[this._played.length - 1];
    if (!last || last.total <= 1) return false;
    return last.index < last.total - 1;
  }

  /**
   * Add a fragment to the queue
   */
  enqueue(fragment: QueuedFragment): void {
    if (fragment.index === 0) {
      this._allFragments = [fragment];
      this._terminal = false;
    } else {
      const exists = this._allFragments.some((f) => f.index === fragment.index);
      if (!exists) {
        this._allFragments.push(fragment);
      }
    }
    this._queue.push(fragment);

    if (this._waitingForNext) {
      this._waitingForNext = false;
      void this.processNext();
    }
  }

  /**
   * Get the current fragment being played (or last played while waiting for next)
   */
  getCurrentFragment(): QueuedFragment | null {
    if (this._queue.length > 0) return this._queue[0];
    if (this._waitingForNext && this._played.length > 0) {
      return this._played[this._played.length - 1];
    }
    return null;
  }

  /**
   * Get the current fragment index (0-based)
   */
  getCurrentIndex(): number | null {
    const fragment = this.getCurrentFragment();
    return fragment ? fragment.index : null;
  }

  /**
   * Get the total number of fragments
   */
  getTotalFragments(): number | null {
    const fragment = this.getCurrentFragment();
    return fragment ? fragment.total : null;
  }

  /**
   * Check if the queue is currently processing or waiting for next fragment
   */
  isProcessing(): boolean {
    return this._isProcessing;
  }

  /**
   * Check if the queue is waiting for a delayed next fragment from backend
   */
  isWaitingForNext(): boolean {
    return this._waitingForNext;
  }

  /**
   * Mark that no more fragments will arrive (e.g. backend synthesis error or abort).
   * If waiting for next fragment, completes queue immediately.
   */
  markTerminal(): void {
    this._terminal = true;
    if (this._waitingForNext) {
      this._waitingForNext = false;
      this._isProcessing = false;
      this._handlers.onQueueComplete();
    }
  }

  /**
   * Check if all fragments for this reading have been received and can be replayed
   */
  canReplay(): boolean {
    return (
      this._allFragments.length > 0 && this._allFragments.length === this._allFragments[0].total
    );
  }

  /**
   * Replay all fragments of the current reading from the beginning
   */
  async replay(): Promise<boolean> {
    if (!this.canReplay()) return false;
    this._queue = [...this._allFragments];
    this._played = [];
    this._terminal = false;
    this._waitingForNext = false;
    this._isProcessing = true;
    await this.processNext();
    return true;
  }

  /**
   * Start processing the queue if not already
   */
  async startProcessing(): Promise<void> {
    if (this._isProcessing) return;

    this._isProcessing = true;
    this._waitingForNext = false;
    await this.processNext();
  }

  /**
   * Process the next fragment in the queue
   */
  private async processNext(): Promise<void> {
    if (this._queue.length === 0) {
      if (this.hasMoreFragmentsExpected() && !this._terminal) {
        this._waitingForNext = true;
        return;
      }
      this._isProcessing = false;
      this._waitingForNext = false;
      this._handlers.onQueueComplete();
      return;
    }

    const next = this._queue[0];
    await this._handlers.onFragmentPlay(next);
  }

  /**
   * Handle when current fragment playback ends
   * Auto-advances to next queued fragment, waits for delayed next fragment,
   * or completes playback when all fragments have finished.
   */
  handleFragmentEnded(): void {
    // Retain the just-completed fragment for cross-boundary rewind
    if (this._queue.length > 0) {
      this._played.push(this._queue.shift()!);
      if (this._played.length > MAX_REWIND_FRAGMENTS) this._played.shift();
    }

    if (this._queue.length > 0) {
      // Auto-advance to next fragment
      void this.processNext();
    } else if (this.hasMoreFragmentsExpected() && !this._terminal) {
      // Current fragment ended, but backend has not delivered subsequent fragments yet.
      // Wait for the next fragment instead of prematurely completing the reading.
      this._waitingForNext = true;
    } else {
      // No more fragments - playback complete
      this._isProcessing = false;
      this._waitingForNext = false;
      this._handlers.onQueueComplete();
    }
  }

  /**
   * Re-queue the most recently played fragment ahead of the current one for a
   * backward skip across the fragment boundary. Returns it, or null when
   * nothing played earlier is still retained. When the returned fragment ends,
   * normal auto-advance resumes with the interrupted fragment.
   */
  previousFragment(): QueuedFragment | null {
    const previous = this._played.pop();
    if (!previous) return null;
    this._queue.unshift(previous);
    this._waitingForNext = false;
    return previous;
  }

  /**
   * Clear the active queue and stop processing (retaining replay fragments if completed)
   */
  clear(): void {
    this._queue = [];
    this._played = [];
    this._isProcessing = false;
    this._waitingForNext = false;
    this._terminal = false;
  }

  /**
   * Fully reset queue state, including replay fragments
   */
  reset(): void {
    this.clear();
    this._allFragments = [];
  }

  /**
   * Get the number of fragments in the queue
   */
  getQueueLength(): number {
    return this._queue.length;
  }

  /**
   * Get all fragments in the queue (read-only)
   */
  getQueue(): readonly QueuedFragment[] {
    return this._queue;
  }
}
