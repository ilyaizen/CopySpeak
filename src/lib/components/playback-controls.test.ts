import { expect, it, vi } from "vite-plus/test";
import { fireEvent, render } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import PlaybackControls from "./playback-controls.svelte";

type ControlsProps = {
  isPlaying?: boolean;
  isPaused?: boolean;
  isSynthesizing?: boolean;
  playMode?: "play" | "replay" | "history" | "disabled";
};

function renderControls(overrides: ControlsProps = {}) {
  const handlers = {
    onPlay: vi.fn(),
    onTogglePause: vi.fn(),
    onStop: vi.fn(),
    onSkipBack: vi.fn(),
    onSkipForward: vi.fn()
  };
  const view = render(PlaybackControls, {
    isPlaying: false,
    isPaused: false,
    isSynthesizing: false,
    playMode: "play",
    ...handlers,
    ...overrides
  });
  return { view, handlers };
}

it("renders the transport row and wires every button", async () => {
  await waitForI18nReady();
  const { view, handlers } = renderControls();
  // SAFETY: every getByRole("button") match in this component is a <Button>,
  // which renders a real <button> element.
  const buttonsOf = () => view.getAllByRole("button") as HTMLButtonElement[];
  expect(buttonsOf()).toHaveLength(4);

  // Idle: skip and stop disabled, the primary button offers Play.
  let buttons = buttonsOf();
  expect(buttons[0].disabled).toBe(true);
  expect(buttons[2].disabled).toBe(true);
  expect(buttons[3].disabled).toBe(true);
  await fireEvent.click(view.getByRole("button", { name: "Play" }));
  expect(handlers.onPlay).toHaveBeenCalledTimes(1);

  // Playing: skips enabled, the primary button becomes Pause, stop enabled.
  await view.rerender({ isPlaying: true });
  buttons = buttonsOf();
  expect(buttons[0].disabled).toBe(false);
  expect(buttons[2].disabled).toBe(false);
  expect(buttons[3].disabled).toBe(false);
  await fireEvent.click(view.getByRole("button", { name: "Back 5 seconds" }));
  expect(handlers.onSkipBack).toHaveBeenCalledTimes(1);
  await fireEvent.click(view.getByRole("button", { name: "Forward 5 seconds" }));
  expect(handlers.onSkipForward).toHaveBeenCalledTimes(1);
  await fireEvent.click(view.getByRole("button", { name: "Pause" }));
  expect(handlers.onTogglePause).toHaveBeenCalledTimes(1);
  await fireEvent.click(view.getByRole("button", { name: "Stop" }));
  expect(handlers.onStop).toHaveBeenCalledTimes(1);

  // Paused: the primary button offers Resume.
  await view.rerender({ isPlaying: false, isPaused: true });
  buttons = buttonsOf();
  expect(buttons[0].disabled).toBe(true);
  expect(buttons[1].disabled).toBe(false);
  await fireEvent.click(view.getByRole("button", { name: "Resume" }));
  expect(handlers.onTogglePause).toHaveBeenCalledTimes(2);

  // Replay mode keeps its tooltip on the primary button (icon-only buttons are
  // named by their tooltips, not the old text labels).
  await view.rerender({ isPaused: false, playMode: "replay" });
  await fireEvent.click(view.getByRole("button", { name: "Replay last audio" }));
  expect(handlers.onPlay).toHaveBeenCalledTimes(2);

  // Synthesizing without audio: the primary button is a disabled spinner (only
  // Stop may act, and Play must not fire a second speak_now).
  await view.rerender({ playMode: "disabled", isSynthesizing: true });
  buttons = buttonsOf();
  expect(view.getAllByRole("button", { name: "Stop" })).toHaveLength(1);
  expect(buttons[1].disabled).toBe(true);
  await fireEvent.click(buttons[3]);
  expect(handlers.onStop).toHaveBeenCalledTimes(2);
});
