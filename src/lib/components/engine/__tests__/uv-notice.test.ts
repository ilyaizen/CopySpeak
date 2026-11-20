import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { cleanup, render, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import { installStore } from "#lib/stores/install-store.svelte";
import UvNotice from "../uv-notice.svelte";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const TITLE = "uv is needed for local engines";

function mockProbe(installed: boolean) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "engine_status") return { installed, voices: [] };
    if (command === "install_engine") return undefined;
    throw new Error("Unexpected command: " + command);
  });
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(async () => {
  await waitForI18nReady();
  invokeMock.mockReset();
});

afterEach(() => {
  cleanup();
  // The store lives for the app lifetime. Drop test runs so each case starts clean.
  Reflect.deleteProperty(installStore.runs, "uv");
  Reflect.deleteProperty(installStore.runs, "kokoro");
});

describe("uv notice", () => {
  it("shows the install action while uv is missing", async () => {
    mockProbe(false);
    const view = render(UvNotice, {});
    expect(await view.findByText(TITLE)).toBeTruthy();
    expect(view.getByRole("button", { name: "Install uv" })).toBeTruthy();
  });

  it("renders nothing once uv is found", async () => {
    mockProbe(true);
    const view = render(UvNotice, {});
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("engine_status", { engine: "uv" }));
    await settle();
    expect(view.queryByRole("status")).toBeNull();
  });

  it("renders nothing when the probe fails", async () => {
    invokeMock.mockRejectedValue(new Error("probe unavailable"));
    const view = render(UvNotice, {});
    await settle();
    expect(view.queryByRole("status")).toBeNull();
  });

  it("shows progress and no button while a uv run is active", async () => {
    mockProbe(false);
    await installStore.start("uv", []);
    const view = render(UvNotice, {});
    expect(await view.findByText("Installing uv…")).toBeTruthy();
    expect(view.queryByRole("button")).toBeNull();
  });

  it("re-checks when a run ends and then shows Failed with Retry", async () => {
    mockProbe(false);
    await installStore.start("uv", []);
    const view = render(UvNotice, {});
    await view.findByText("Installing uv…");
    installStore.fail("uv", "[ERROR] install did not start: boom");
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([cmd]) => cmd === "engine_status")).toHaveLength(2)
    );
    expect(await view.findByText("Install failed.")).toBeTruthy();
    expect(view.getByText("The uv installer did not start. Check the install log.")).toBeTruthy();
    expect(view.getByRole("button", { name: "Retry" })).toBeTruthy();
  });

  it("disables Install while another engine run is active", async () => {
    mockProbe(false);
    await installStore.start("kokoro", []);
    const view = render(UvNotice, {});
    const button = await view.findByRole("button", { name: "Install uv" });
    expect(button.hasAttribute("disabled")).toBe(true);
  });
});
