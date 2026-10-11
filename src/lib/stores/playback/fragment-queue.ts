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
  private _isProcessing = false;
  private _handlers: FragmentQueueHandlers;

  constructor(handlers: FragmentQueueHandlers) {
    this._handlers = handlers;
  }

  /**
   * Add a fragment to the queue
   */
  enqueue(fragment: QueuedFragment): void {
    this._queue.push(fragment);
  }

  /**
   * Get the current fragment being played
   */
  getCurrentFragment(): QueuedFragment | null {
    return this._queue.length > 0 ? this._queue[0] : null;
  }

  /**
   * Get the current fragment index (1-based)
   */
  getCurrentIndex(): number | null {
    return this._queue.length > 0 ? this._queue[0].index : null;
  }

  /**
   * Get the total number of fragments
   */
  getTotalFragments(): number | null {
    return this._queue.length > 0 ? this._queue[0].total : null;
  }

  /**
   * Check if the queue is currently processing
   */
  isProcessing(): boolean {
    return this._isProcessing;
  }

  /**
   * Start processing the queue if not already
   */
  async startProcessing(): Promise<void> {
    if (this._isProcessing) return;

    this._isProcessing = true;
    await this.processNext();
  }

  /**
   * Process the next fragment in the queue
   */
  private async processNext(): Promise<void> {
    if (this._queue.length === 0) {
      this._isProcessing = false;
      return;
    }

    const next = this._queue[0];
    await this._handlers.onFragmentPlay(next);
  }

  /**
   * Handle when current fragment playback ends
   * Auto-advances to next queued fragment or stops if queue is empty
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
    } else {
      // No more fragments - playback complete
      this._isProcessing = false;
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
    return previous;
  }

  /**
   * Clear the queue and stop processing
   */
  clear(): void {
    this._queue = [];
    this._played = [];
    this._isProcessing = false;
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
