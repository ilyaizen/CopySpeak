import { test, expect } from "@playwright/test";
import path from "node:path";

test("settings persist and website access can be granted, refused, and removed", async ({
  page
}) => {
  await page.route("http://localhost/**", (route) => {
    const asset = new URL(route.request().url()).pathname.slice(1);
    if (
      !["options.html", "options.css", "options.js", "icons/128.png", "icons/32.png"].includes(
        asset
      )
    )
      return route.abort();
    return route.fulfill({ path: path.resolve("browser-extension/dist", asset) });
  });
  await page.addInitScript(() => {
    // SAFETY: this isolated page fixture owns the chrome stub and settings state.
    const w = window as any;
    const settings: Record<string, boolean> = {};
    let origins: string[] = [];
    const listeners: (() => void)[] = [];
    const event = { addListener: (listener: () => void) => listeners.push(listener) };
    w.settings = settings;
    w.chrome = {
      runtime: {
        getManifest: () => ({ version: "0.1.1" }),
        sendMessage: async () => ({ ok: true })
      },
      storage: {
        local: {
          get: async () => settings,
          set: async (value: Record<string, boolean>) => {
            Object.assign(settings, value);
            listeners.forEach((listener) => listener());
          }
        },
        onChanged: event
      },
      permissions: {
        getAll: async () => ({ origins }),
        request: async (request: { origins: string[] }) => {
          if (request.origins.includes("http://*/*")) return false;
          origins = [...new Set([...origins, ...request.origins])];
          return true;
        },
        remove: async (request: { origins: string[] }) => {
          origins = origins.filter((origin) => !request.origins.includes(origin));
          return true;
        },
        onAdded: event,
        onRemoved: event
      }
    };
  });
  await page.goto("http://localhost/options.html");
  await expect(page.locator("#version")).toHaveText("0.1.1");
  await expect(page.getByLabel("Show the playback box")).toBeChecked();
  await page.getByLabel("Hover to read a paragraph").check();
  await page.getByLabel("Show the playback box").uncheck();
  await expect
    .poll(() =>
      page.evaluate(() => {
        // SAFETY: settings is installed by this test's init script.
        return (window as any).settings;
      })
    )
    .toEqual({ hoverRead: true, showPanel: false });
  await page.getByLabel("Website address").fill("example.com/article");
  await page.getByRole("button", { name: "Allow website", exact: true }).click();
  await expect(page.locator("#sites")).toContainText("https://example.com/*");
  await page.getByRole("button", { name: "Allow all websites…", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Website access was not granted");
  await expect(page.locator("#sites li")).toHaveCount(1);
  await page.getByRole("button", { name: "Remove access to https://example.com/*" }).click();
  await expect(page.locator("#sites")).toContainText("No websites allowed yet");
  await expect(page.getByLabel("Hover to read a paragraph")).toBeChecked();
  await expect(page.getByLabel("Show the playback box")).not.toBeChecked();
});
