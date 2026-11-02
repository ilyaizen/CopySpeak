import { afterEach, expect, it, vi } from "vite-plus/test";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { setLocale, waitForI18nReady } from "#lib/i18n";
import { isRtl } from "#lib/i18n/store";
import { get } from "svelte/store";
import InfoTooltip from "./info-tooltip.svelte";

afterEach(() => {
  cleanup();
  setLocale("en");
  vi.unstubAllGlobals();
});

it("renders the floating tooltip in the current RTL locale", async () => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    }
  );
  await waitForI18nReady();
  setLocale("he");
  await waitForI18nReady();
  await waitFor(() => expect(get(isRtl)).toBe(true));
  render(InfoTooltip, { text: "הפעל את CopySpeak כשאתה מתחבר" });
  await fireEvent.focus(screen.getByRole("button"));
  const text = await screen.findByText("הפעל את CopySpeak כשאתה מתחבר");
  const wrapper = text.closest("[data-bits-floating-content-wrapper]");
  expect(wrapper?.getAttribute("dir")).toBe("rtl");
  setLocale("en");
  await waitFor(() => expect(wrapper?.getAttribute("dir")).toBe("ltr"));
  setLocale("ar");
  await waitFor(() => expect(wrapper?.getAttribute("dir")).toBe("rtl"));
});
