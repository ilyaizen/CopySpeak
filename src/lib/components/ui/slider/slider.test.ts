import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import Slider from "./slider.svelte";

afterEach(() => {
  cleanup();
});

describe("Slider", () => {
  it("renders with squarish brutalist grabber styling (rounded-sm)", () => {
    const { container } = render(Slider, { value: 50, min: 0, max: 100 });
    const thumb = container.querySelector('[data-slot="slider-thumb"]');
    expect(thumb).not.toBeNull();
    expect(thumb?.className).toContain("rounded-sm");
    expect(thumb?.className).not.toContain("rounded-full");
    expect(thumb?.getAttribute("style")).toContain("inset-inline-start: calc(50% - 6px)");
  });

  it("positions grabber according to the value percentage", () => {
    const { container } = render(Slider, { value: 25, min: 0, max: 100 });
    const thumb = container.querySelector('[data-slot="slider-thumb"]');
    expect(thumb?.getAttribute("style")).toContain("inset-inline-start: calc(25% - 6px)");
  });

  it("fires oninput and onchange with snapped values", async () => {
    const oninput = vi.fn();
    const onchange = vi.fn();
    const { container } = render(Slider, {
      value: 1,
      min: 0.5,
      max: 2,
      step: 0.05,
      oninput,
      onchange
    });

    const input = container.querySelector('input[type="range"]');
    if (!(input instanceof HTMLInputElement)) throw new Error("Missing slider input");

    input.value = "1.234";
    await fireEvent.input(input);
    expect(oninput).toHaveBeenCalledWith(1.25);

    input.value = "1.789";
    await fireEvent.input(input);
    await fireEvent.change(input);
    expect(onchange).toHaveBeenCalledWith(1.8);
  });

  it("propagates disabled state to the input", () => {
    const { container } = render(Slider, { value: 50, disabled: true });
    const input = container.querySelector('input[type="range"]');
    if (!(input instanceof HTMLInputElement)) throw new Error("Missing slider input");
    expect(input.disabled).toBe(true);
  });
});
