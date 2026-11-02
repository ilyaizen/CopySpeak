import { afterEach, expect, it } from "vite-plus/test";
import { cleanup, render } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import { testConfig } from "#lib/test-fixtures/app-config";
import QuickSettings from "./quick-settings.svelte";

afterEach(() => cleanup());

it("shows the quick hotkey only when Experimental and the hotkey are enabled", async () => {
  await waitForI18nReady();
  for (const experimental of [false, true]) {
    for (const enabled of [false, true]) {
      const config = testConfig();
      config.general.experimental_enabled = experimental;
      config.hotkey.enabled = enabled;
      const view = render(QuickSettings, { config });
      expect(Boolean(view.container.querySelector("#qs-hotkey"))).toBe(experimental && enabled);
      view.unmount();
    }
  }
});
