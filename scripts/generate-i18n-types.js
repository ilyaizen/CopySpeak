#!/usr/bin/env node

/**
 * Generate TypeScript types from en.json translation file
 * Run: node scripts/generate-i18n-types.js
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const EN_JSON_PATH = path.join(__dirname, "..", "src", "lib", "locales", "en.json");
const OUTPUT_PATH = path.join(__dirname, "..", "src", "lib", "i18n", "types.ts");

/**
 * Flatten nested JSON object into dot-notation keys
 */
function flattenKeys(obj, prefix = "") {
  let keys = [];

  for (const key in obj) {
    if (obj.hasOwnProperty(key)) {
      const newKey = prefix ? `${prefix}.${key}` : key;

      if (typeof obj[key] === "object" && obj[key] !== null) {
        keys = keys.concat(flattenKeys(obj[key], newKey));
      } else {
        keys.push(newKey);
      }
    }
  }

  return keys;
}

/**
 * Generate TypeScript union type from keys
 */
function generateTypes(keys) {
  const keyStrings = keys.map((key) => `  | "${key}"`).join("\n");

  return (
    `// Auto-generated from en.json by scripts/generate-i18n-types.js
// Do not edit manually - run the script to regenerate

/**
 * All available translation keys
 * Use with \$_() from svelte-i18n for type-safe translations
 * 
 * Example:
 * \`\`\`svelte
 * <script>
 *   import { _ } from 'svelte-i18n';
 *   import type { TranslationKeys } from '$lib/i18n/types';
 *   
 *   const key: TranslationKeys = 'settings.categories.general';
 * </script>
 * 
 * <h1>{\$_(key)}</h1>
 * \`\`\`
 */
export type TranslationKeys =
${keyStrings};

/**
 * Helper type for nested key paths
 * Use to restrict keys to a specific namespace
 * 
 * Example:
 * \`\`\`typescript
 * type SettingsKeys = NestedKeyOf<TranslationKeys, 'settings'>;
 * // Results in: 'settings.categories.general' | 'settings.categories.playback' | ...
 * \`\`\`
 */
export type NestedKeyOf<T extends string, Prefix extends string> = 
  T extends ` +
    "`${Prefix}.${infer _Rest}`" +
    ` ? T : never;
`
  );
}

function main() {
  console.log("Generating i18n types from en.json...");

  try {
    // Read en.json
    const enJsonContent = fs.readFileSync(EN_JSON_PATH, "utf-8");
    const enJson = JSON.parse(enJsonContent);

    // Flatten keys
    const keys = flattenKeys(enJson);
    console.log(`Found ${keys.length} translation keys`);

    // Generate TypeScript
    const typesContent = generateTypes(keys);

    // Write output
    fs.writeFileSync(OUTPUT_PATH, typesContent, "utf-8");

    console.log(`✓ Generated types at ${OUTPUT_PATH}`);
    console.log(`  Total keys: ${keys.length}`);
  } catch (error) {
    console.error("Error generating types:", error.message);
    process.exit(1);
  }
}

main();
