import { describe, expect, test } from "vite-plus/test";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

// The row contract: every SettingRow carries a tooltip. Descriptions live in
// the tooltip, never in a <p> under the row, so the sidebar layout reads
// clean and the info is one hover away everywhere.
// This file lives in settings/__tests__/, so the source folder is one level up.
// Fail loudly if that folder yields no components: an empty list would
// generate zero tests and pass silently.
const SETTINGS_DIR = join(import.meta.dirname, "..");

function settingComponentPaths(): string[] {
  const paths = readdirSync(SETTINGS_DIR)
    .filter((name) => name.endsWith(".svelte"))
    .map((name) => join(SETTINGS_DIR, name));
  if (paths.length === 0) {
    throw new Error(
      `No .svelte components found in ${SETTINGS_DIR}; the tooltip contract would test nothing`
    );
  }
  return paths;
}

function settingRowsWithoutTooltip(source: string): string[] {
  const rows: string[] = [];
  const pattern = /<SettingRow\b/g;
  for (const match of source.matchAll(pattern)) {
    const start = match.index;
    // Scan to the tag's closing '>' while ignoring '>' inside {...} expressions.
    let depth = 0;
    let index = start;
    while (index < source.length) {
      const ch = source[index];
      if (ch === "{") depth += 1;
      else if (ch === "}") depth = Math.max(0, depth - 1);
      else if (ch === ">" && depth === 0) break;
      index += 1;
    }
    rows.push(source.slice(start, index + 1));
  }
  return rows.filter((row) => !row.includes("tooltip="));
}

describe("SettingRow tooltip contract", () => {
  for (const path of settingComponentPaths()) {
    test(`${path.split(/[\\/]/).pop()} puts a tooltip on every SettingRow`, () => {
      const source = readFileSync(path, "utf-8");
      expect(settingRowsWithoutTooltip(source)).toEqual([]);
    });
  }
});
