export {};

const status = document.querySelector<HTMLElement>("#status")!;
const sites = document.querySelector<HTMLUListElement>("#sites")!;
const site = document.querySelector<HTMLInputElement>("#site")!;
const allSites = ["http://*/*", "https://*/*"];
document.querySelector<HTMLElement>("#version")!.textContent = chrome.runtime.getManifest().version;

async function apply() {
  const result = await chrome.runtime.sendMessage({ type: "settings" });
  if (!result?.ok) throw new Error(result?.error ?? "Could not apply settings. Try again.");
}
async function refresh() {
  const settings = await chrome.storage.local.get(["hoverRead", "automatic", "showPanel"]);
  for (const key of ["hoverRead", "automatic", "showPanel"]) {
    document.querySelector<HTMLInputElement>(`#${key}`)!.checked =
      key === "showPanel" ? settings[key] !== false : settings[key] === true;
  }
  const permissions = await chrome.permissions.getAll();
  sites.replaceChildren();
  for (const origin of permissions.origins ?? []) {
    const item = document.createElement("li");
    const name = document.createElement("span");
    name.textContent = origin;
    const remove = document.createElement("button");
    remove.textContent = "Remove";
    remove.setAttribute("aria-label", `Remove access to ${origin}`);
    remove.onclick = () => {
      void save(async () => {
        await chrome.permissions.remove({ origins: [origin] });
        await apply();
      });
    };
    item.append(name, remove);
    sites.append(item);
  }
  if (!sites.children.length) {
    const item = document.createElement("li");
    item.textContent =
      "No websites allowed yet. Add one to use hover reading or double-copy highlighting.";
    sites.append(item);
  }
}
async function save(action: () => Promise<void>) {
  try {
    await action();
    status.textContent = "Saved. Changes apply to open pages.";
  } catch (error) {
    status.textContent = error instanceof Error ? error.message : "Could not save. Try again.";
  }
  await refresh();
}
for (const key of ["hoverRead", "automatic", "showPanel"]) {
  const input = document.querySelector<HTMLInputElement>(`#${key}`)!;
  input.onchange = () => {
    const checked = input.checked;
    void save(async () => {
      await chrome.storage.local.set({ [key]: checked });
      await apply();
    });
  };
}
function grant(origins: string[]) {
  // Request directly within the click/submit gesture, before any storage awaits.
  const permission = chrome.permissions.request({ origins });
  void save(async () => {
    if (!(await permission))
      throw new Error("Website access was not granted. Your settings are unchanged.");
    await apply();
  });
}
document.querySelector<HTMLFormElement>("#site-form")!.onsubmit = (event) => {
  event.preventDefault();
  try {
    const value = site.value.trim();
    const url = new URL(value.includes("://") ? value : `https://${value}`);
    if (!/^https?:$/.test(url.protocol) || url.username || url.password)
      throw new Error("Enter an ordinary http or https website address.");
    grant([`${url.protocol}//${url.hostname}/*`]);
  } catch {
    status.textContent = "Enter an ordinary website address, such as https://example.com.";
  }
};
document.querySelector<HTMLButtonElement>("#all-sites")!.onclick = () => grant(allSites);
chrome.storage.onChanged.addListener(() => {
  void refresh();
});
chrome.permissions.onRemoved.addListener(() => {
  void refresh();
});
chrome.permissions.onAdded.addListener(() => {
  void refresh();
});
void refresh().catch(() => {
  status.textContent = "Could not load settings. Reopen this page to retry.";
});
