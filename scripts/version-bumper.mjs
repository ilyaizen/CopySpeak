import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

// --- Configuration ---
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..");

const versionFile = "src/lib/version.ts";

const filesToUpdate = {
  "package.json": (content, version) => content.replace(/("version":\s*)"[^"]+"/, `$1"${version}"`),
  "src-tauri/tauri.conf.json": (content, version) =>
    content.replace(/("version":\s*)"[^"]+"/, `$1"${version}"`),
  "src-tauri/Cargo.toml": (content, version) =>
    content.replace(/(version\s*=\s*)"[^"]+"/, `$1"${version}"`),
  "src-tauri/src/config/mod.rs": (content, version) =>
    content.replace(/(const CONFIG_VERSION: &str = ")[^"]+(")/, `$1${version}$2`),
  [versionFile]: (content, version) =>
    content.replace(/export const VERSION = "[^"]+";/, `export const VERSION = "${version}";`)
};

// --- Script ---
async function bumpVersion() {
  try {
    console.log("Starting version bump process...");

    // 1. Read current version from version.ts
    const versionFilePath = path.join(projectRoot, versionFile);
    const versionFileContent = await fs.readFile(versionFilePath, "utf-8");
    const versionMatch = versionFileContent.match(/export const VERSION = "([^"]+)";/);
    if (!versionMatch) {
      throw new Error("Could not find VERSION in version.ts");
    }
    const currentVersion = versionMatch[1];
    console.log(`Current version: ${currentVersion}`);

    // 2. Determine new version from command line argument
    const bumpType = process.argv[2] || "patch"; // 'major', 'minor', or 'patch'
    const versionParts = currentVersion.split(".").map(Number);

    switch (bumpType) {
      case "major":
        versionParts[0]++;
        versionParts[1] = 0;
        versionParts[2] = 0;
        break;
      case "minor":
        versionParts[1]++;
        versionParts[2] = 0;
        break;
      case "patch":
        versionParts[2]++;
        break;
      default:
        console.error(`Invalid bump type "${bumpType}". Use 'major', 'minor', or 'patch'.`);
        process.exit(1);
    }
    const newVersion = versionParts.join(".");
    console.log(`Bumping version to ${newVersion}`);

    // 3. Update all configured files
    for (const [filePath, updater] of Object.entries(filesToUpdate)) {
      const fullPath = path.join(projectRoot, filePath);
      try {
        const content = await fs.readFile(fullPath, "utf-8");
        const newContent = updater(content, newVersion);
        if (content !== newContent) {
          await fs.writeFile(fullPath, newContent);
          console.log(`✅ Updated ${filePath}`);
        } else {
          console.log(`⚠️ No version string found in ${filePath}. Skipping.`);
        }
      } catch (fileError) {
        console.error(`❌ Error processing file ${filePath}:`, fileError.message);
      }
    }

    console.log("Version bump complete!");
  } catch (error) {
    console.error("❌ Fatal error during version bump:", error);
    process.exit(1);
  }
}

bumpVersion();
