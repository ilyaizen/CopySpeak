import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import { testConfig } from "#lib/test-fixtures/app-config";
import { ALL_SETUP_ENTRIES, CLOUD_ENGINES, LOCAL_ENGINES } from "../engine-meta";
import EngineSetup from "../engine-setup.svelte";
import { installStore } from "#lib/stores/install-store.svelte";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

beforeEach(async () => {
  await waitForI18nReady();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "engine_runtime_status") return { installed: false, provider: null };
    if (command === "engine_status") return { installed: true, voices: [] };
    if (command === "test_tts_engine_config") return { success: true, message: "ok" };
    throw new Error(`Unexpected command: ${command}`);
  });
});
afterEach(() => {
  cleanup();
  for (const run of Object.values(installStore.runs)) run.running = false;
});

describe("engine configuration dashboard", () => {
  it("shows all engines without a sidebar and filters cloud and local rows", async () => {
    const view = render(EngineSetup, {
      localConfig: testConfig(),
      isDirty: false,
      onSave: vi.fn()
    });
    expect(view.queryByRole("navigation")).toBeNull();
    expect(view.getAllByRole("row")).toHaveLength(ALL_SETUP_ENTRIES.length + 1);
    await fireEvent.click(view.getByRole("button", { name: "Local" }));
    expect(view.getAllByRole("row")).toHaveLength(LOCAL_ENGINES.length + 1);
    await fireEvent.click(view.getByRole("button", { name: "Cloud" }));
    expect(view.getAllByRole("row")).toHaveLength(CLOUD_ENGINES.length + 1);
    expect(
      invokeMock.mock.calls.every(
        ([cmd, args]) =>
          cmd === "engine_runtime_status" || (cmd === "engine_status" && args?.engine === "uv")
      )
    ).toBe(true);
  });

  it("masks keys, gives every field a unique ID, and uses environment sentinels", async () => {
    const config = testConfig();
    config.tts.openai.api_key = "test-key";
    const view = render(EngineSetup, {
      localConfig: config,
      isDirty: false,
      onSave: vi.fn(),
      envKeySources: { fish: "FISH_API_KEY" }
    });
    const input = view.getByLabelText("OpenAI API Key");
    if (!(input instanceof HTMLInputElement)) throw new Error("Expected an API key input");
    expect(input.type).toBe("password");
    await fireEvent.click(view.getByRole("button", { name: "OpenAI: Show API key" }));
    expect(input.type).toBe("text");
    const fields = [...view.container.querySelectorAll("input")];
    expect(new Set(fields.map((field) => field.id)).size).toBe(fields.length);
    expect(view.getByText("FISH_API_KEY", { exact: false })).toBeTruthy();
    expect(view.queryByRole("button", { name: /Fish.*Show API key/ })).toBeNull();
    expect(view.getByRole("button", { name: /Test Fish/ }).hasAttribute("disabled")).toBe(false);
    expect(
      view.getByRole("button", { name: "Test Microsoft Foundry" }).hasAttribute("disabled")
    ).toBe(true);
  });

  it("waits for saving before testing and blocks other tests during that wait", async () => {
    const config = testConfig();
    config.tts.openai.api_key = "test-key";
    let finishSave!: () => void;
    const onSave = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishSave = resolve;
        })
    );
    const view = render(EngineSetup, { localConfig: config, isDirty: true, onSave });
    await fireEvent.click(view.getByRole("button", { name: "Test OpenAI" }));
    expect(onSave).toHaveBeenCalledOnce();
    expect(invokeMock).not.toHaveBeenCalledWith("test_tts_engine_config", expect.anything());
    expect(view.getByRole("button", { name: "Test OpenAI" }).hasAttribute("disabled")).toBe(true);
    finishSave();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("test_tts_engine_config", { engine: "openai" })
    );
    await waitFor(() => expect(view.getByText("Engine Test Passed")).toBeTruthy());
    await fireEvent.input(view.getByLabelText("OpenAI API Key"), {
      target: { value: "replacement" }
    });
    expect(view.queryByText("Engine Test Passed")).toBeNull();
  });

  it("does not test when saving fails or awaits an environment warning", async () => {
    const config = testConfig();
    config.tts.openai.api_key = "test-key";
    const onSave = vi.fn().mockRejectedValue(new Error("Save deferred"));
    const view = render(EngineSetup, { localConfig: config, isDirty: true, onSave });
    await fireEvent.click(view.getByRole("button", { name: "Test OpenAI" }));
    await waitFor(() =>
      expect(view.getByRole("button", { name: "Test OpenAI" }).hasAttribute("disabled")).toBe(false)
    );
    expect(invokeMock).not.toHaveBeenCalledWith("test_tts_engine_config", expect.anything());
  });

  it("reports failed runtime probes without claiming an engine is uninstalled", async () => {
    invokeMock.mockRejectedValue(new Error("Probe unavailable"));
    const view = render(EngineSetup, {
      localConfig: testConfig(),
      isDirty: false,
      onSave: vi.fn()
    });
    await waitFor(() =>
      expect(view.getAllByText("Status unavailable")).toHaveLength(LOCAL_ENGINES.length)
    );
    expect(view.queryByText("Not installed")).toBeNull();
    expect(view.getByRole("button", { name: "Test Kokoro TTS" }).hasAttribute("disabled")).toBe(
      true
    );
  });

  it("keeps the test lock when an installer starts and settles during a request", async () => {
    const config = testConfig();
    config.tts.openai.api_key = "test-key";
    let finishTest!: (result: { success: boolean; message: string }) => void;
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "engine_runtime_status") return { installed: true, provider: "cpu" };
      if (command === "test_tts_engine_config") {
        return new Promise<{ success: boolean; message: string }>((resolve) => {
          finishTest = resolve;
        });
      }
      throw new Error(`Unexpected command: ${command}`);
    });
    const view = render(EngineSetup, { localConfig: config, isDirty: false, onSave: vi.fn() });
    await fireEvent.click(view.getByRole("button", { name: "Test OpenAI" }));
    await installStore.start("kokoro", []);
    await waitFor(() => expect(view.getByText("Install in progress")).toBeTruthy());
    const run = installStore.run("kokoro");
    if (!run) throw new Error("Expected an installer run");
    run.running = false;
    await waitFor(() => expect(view.queryByText("Install in progress")).toBeNull());
    expect(view.getByRole("button", { name: "Test Edge TTS" }).hasAttribute("disabled")).toBe(true);
    finishTest({ success: true, message: "ok" });
    await waitFor(() =>
      expect(view.getByRole("button", { name: "Test OpenAI" }).hasAttribute("disabled")).toBe(false)
    );
  });

  it("discards pending results when the save bar replaces credential configuration", async () => {
    const config = testConfig();
    config.tts.openai.api_key = "test-key";
    let finishTest!: (result: { success: boolean; message: string }) => void;
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "engine_runtime_status") return { installed: false, provider: null };
      if (command === "test_tts_engine_config") {
        return new Promise<{ success: boolean; message: string }>((resolve) => {
          finishTest = resolve;
        });
      }
      throw new Error(`Unexpected command: ${command}`);
    });
    const view = render(EngineSetup, { localConfig: config, isDirty: false, onSave: vi.fn() });
    await fireEvent.click(view.getByRole("button", { name: "Test OpenAI" }));
    await view.rerender({ localConfig: testConfig() });
    expect(view.getByRole("button", { name: "Test Edge TTS" }).hasAttribute("disabled")).toBe(true);
    finishTest({ success: false, message: "Stale provider error" });
    await waitFor(() =>
      expect(view.getByRole("button", { name: "Test Edge TTS" }).hasAttribute("disabled")).toBe(
        false
      )
    );
    expect(view.queryByText("Stale provider error")).toBeNull();
    expect(view.queryByText("Engine Test Failed")).toBeNull();
  });

  it("clears a completed result when Cancel replaces the configuration", async () => {
    const config = testConfig();
    config.tts.openai.api_key = "test-key";
    const view = render(EngineSetup, { localConfig: config, isDirty: false, onSave: vi.fn() });
    await fireEvent.click(view.getByRole("button", { name: "Test OpenAI" }));
    await waitFor(() => expect(view.getByText("Engine Test Passed")).toBeTruthy());
    await view.rerender({ localConfig: testConfig() });
    expect(view.queryByText("Engine Test Passed")).toBeNull();
  });

  it("uses one-shot synthesis for installed local engines without saving cloud keys", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "engine_runtime_status") return { installed: true, provider: "cpu" };
      if (command === "test_local_engine") return { success: false, message: "Missing model" };
      throw new Error(`Unexpected command: ${command}`);
    });
    const onSave = vi.fn();
    const view = render(EngineSetup, { localConfig: testConfig(), isDirty: true, onSave });
    const button = view.getByRole("button", { name: "Test Kokoro TTS" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    await fireEvent.click(button);
    await waitFor(() => expect(view.getByText("Missing model")).toBeTruthy());
    expect(onSave).not.toHaveBeenCalled();
    expect(invokeMock).toHaveBeenCalledWith("test_local_engine", { engine: "kokoro" });
  });
});
