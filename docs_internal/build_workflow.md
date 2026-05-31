# Build Workflow Documentation

This document describes the GitHub Actions workflow for building CopySpeak Windows installers.

## Workflow: Build Windows Installer

**File:** [`.github/workflows/build-windows.yml`](.github/workflows/build-windows.yml)

### Overview

The workflow automatically builds a pre-production Windows 11 installer for CopySpeak and publishes it to a GitHub release.

### Triggering the Workflow

The workflow triggers automatically on **push to `main`** branch:

1. **Automatic (push to main):**
   - Build runs and creates a draft release
   - Release name: "CopySpeak v{version}" (e.g., "CopySpeak v0.0.8")

2. **Manual (workflow_dispatch):**
   - Go to **Actions** tab in your GitHub repository
   - Select **Build Windows Installer** workflow
   - Click **Run workflow**
   - Fill in the optional parameter:
     - **tag**: Release tag (uses current version if empty, e.g., `v0.0.8`)

### Version Management

The version is read from [`package.json`](../package.json) and used to create the release tag. The version is synchronized across multiple files by the version bumper script:

- [`package.json`](../package.json) - Frontend package version (source of truth)
- [`src-tauri/Cargo.toml`](../src-tauri/Cargo.toml) - Rust package version
- [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json) - App version
- [`src/lib/version.ts`](../src/lib/version.ts) - Runtime version constant

**To bump the version before pushing:**

```bash
# Patch version (0.0.x)
bun run bump

# Minor version (0.x.0)
bun run bump:minor

# Major version (x.0.0)
bun run bump:major
```

The version bumper script ([`scripts/version-bumper.mjs`](../scripts/version-bumper.mjs)) automatically updates all version references across the project.

### Workflow Steps

1. **Checkout repository** - Clones the code
2. **Get version from package.json** - Reads version from [`package.json`](../package.json)
3. **Set release tag** - Creates release tag (auto-detected or custom)
4. **Setup Bun** - Installs Bun runtime
5. **Install frontend dependencies** - Runs `bun install`
6. **Build frontend** - Runs `bun run build`
7. **Setup Rust toolchain** - Installs stable Rust
8. **Rust cache** - Caches Rust dependencies for faster builds
9. **Build Tauri app** - Uses `tauri-action` to build and upload artifacts

### Output

The workflow will:

- Build a Windows installer (`.msi` or `.exe`)
- Create a GitHub release with the version tag
- Upload the installer as a release asset
- Mark the release as a draft (requires manual review before publishing)

### Release Artifacts

Built artifacts will be attached to the GitHub release:

- `CopySpeak_0.0.7_x64-setup.exe` - Windows installer
- `CopySpeak_0.0.7_x64-setup.exe.sig` - Signature file (if configured)
- `bundle.log` - Build log

### Permissions

The workflow requires `contents: write` permission to create releases and upload artifacts.

### Troubleshooting

**Build fails:**

- Check the Actions logs for specific error messages
- Ensure all dependencies are properly installed
- Verify that the version numbers are consistent across all files

**Release not created:**

- Ensure the repository has the correct permissions
- Check that `GITHUB_TOKEN` has write access to contents
- Verify the tag format (should start with `v`)

**Version mismatch:**

- Ensure you've run the version bumper script before pushing
- Check that [`package.json`](../package.json), [`src-tauri/Cargo.toml`](../src-tauri/Cargo.toml), and [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json) all have the same version
- Run `bun run bump` (or `bun run bump:minor`/`bun run bump:major`) to synchronize versions

**Installer fails on Windows:**

- Ensure Windows SDK is properly configured in Tauri
- Check that all Windows-specific dependencies are included
- Review the `bundle.log` for detailed build information

### Next Steps

After a successful build:

1. Review the draft release
2. Test the installer on a clean Windows 11 machine
3. Update the release notes with specific changes
4. Publish the release when ready

### Recommended Workflow

1. Run `bun run bump` (or `bun run bump:minor`/`bun run bump:major`) to increment version
2. Commit and push the version changes to `main`
3. The workflow automatically builds and creates a draft release
4. Review and publish the release

### Related Documentation

- [Tauri Action Documentation](https://github.com/tauri-apps/tauri-action)
- [Tauri Building Guide](https://tauri.app/v1/guides/building/)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
