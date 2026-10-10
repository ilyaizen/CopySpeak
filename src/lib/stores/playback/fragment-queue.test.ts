import { describe, expect, it } from "vite-plus/test";
import { FragmentQueue, type QueuedFragment } from "./fragment-queue";

function fragment(index: number): QueuedFragment {
  return { audioBase64: `audio-${index}`, index, total: 5, text: `Part ${index}` };
}

function createQueue() {
  const played: number[] = [];
  const queue = new FragmentQueue({
    onFragmentPlay: async (fragment) => {
      played.push(fragment.index);
    },
    onQueueComplete: () => {}
  });
  return { queue, played };
}

describe("FragmentQueue", () => {
  it("retains played fragments for rewind and re-queues them ahead of current", async () => {
    const { queue, played } = createQueue();
    for (const index of [0, 1, 2]) queue.enqueue(fragment(index));
    await queue.startProcessing();
    expect(played).toEqual([0]);
    queue.handleFragmentEnded();
    expect(played).toEqual([0, 1]);
    expect(queue.getCurrentIndex()).toBe(1);
    const previous = queue.previousFragment();
    expect(previous?.index).toBe(0);
    expect(queue.getQueue().map((f) => f.index)).toEqual([0, 1, 2]);
  });

  it("returns null when nothing played earlier is retained", async () => {
    const { queue } = createQueue();
    queue.enqueue(fragment(0));
    await queue.startProcessing();
    expect(queue.previousFragment()).toBeNull();
    // The queue is untouched when there is nothing to step back into.
    expect(queue.getQueue().map((f) => f.index)).toEqual([0]);
  });

  it("caps the rewind history at three fragments", async () => {
    const { queue } = createQueue();
    for (let index = 0; index < 5; index++) queue.enqueue(fragment(index));
    await queue.startProcessing();
    for (let i = 0; i < 4; i++) queue.handleFragmentEnded();
    expect(queue.getCurrentIndex()).toBe(4);
    // Fragments 0..3 played; only 1..3 fit the cap, so 0 was evicted.
    expect(queue.previousFragment()?.index).toBe(3);
  });

  it("clear drops the rewind history along with the queue", async () => {
    const { queue } = createQueue();
    queue.enqueue(fragment(0));
    queue.enqueue(fragment(1));
    await queue.startProcessing();
    queue.handleFragmentEnded();
    queue.clear();
    expect(queue.previousFragment()).toBeNull();
    expect(queue.getQueueLength()).toBe(0);
  });

  it("auto-advances through a re-queued rewind target back to the interrupted fragment", async () => {
    const { queue, played } = createQueue();
    for (const index of [0, 1, 2]) queue.enqueue(fragment(index));
    await queue.startProcessing(); // plays 0
    queue.handleFragmentEnded(); // 0 done, plays 1
    queue.previousFragment(); // re-queues 0 ahead of 1; the store plays it itself
    queue.handleFragmentEnded(); // rewind target 0 done, plays the interrupted 1 again
    expect(played).toEqual([0, 1, 1]);
    queue.handleFragmentEnded(); // 1 done, plays 2
    expect(played).toEqual([0, 1, 1, 2]);
    expect(queue.getCurrentIndex()).toBe(2);
  });
});
