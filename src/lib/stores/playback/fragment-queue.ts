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
}

export interface FragmentQueueHandlers {
  onFragmentPlay: (fragment: QueuedFragment) => Promise<void>;
  onQueueComplete: () => void;
}

export class FragmentQueue {
  private _queue: QueuedFragment[] = [];
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
    // Remove the just-completed fragment from queue
    if (this._queue.length > 0) {
      this._queue.shift();
    }

    if (this._queue.length > 0) {
      // Auto-advance to next fragment
      this.processNext();
    } else {
      // No more fragments - playback complete
      this._isProcessing = false;
      this._handlers.onQueueComplete();
    }
  }

  /**
   * Clear the queue and stop processing
   */
  clear(): void {
    this._queue = [];
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
