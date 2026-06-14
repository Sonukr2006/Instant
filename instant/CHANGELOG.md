# Changelog

## Unreleased - 2026-06-14

### Fixed
- HMR runtime crash for arrow-function/default exports
  - Added `instant/scripts/patch-refresh-runtime.js` which patches `node_modules/@vitejs/plugin-react/dist/refresh-runtime.js` to guard unsafe `type.prototype` access and avoid `TypeError` during HMR. The script runs automatically via `postinstall` in `instant/package.json` or can be run manually.

- Windows selected-text capture race condition
  - Made selected-text copy and shortcut-release timeouts configurable via environment variables and increased sensible defaults.
  - Added warnings when shortcut-release or clipboard-copy timeouts occur to aid telemetry and debugging.
  - Files changed: `instant/src-tauri/src/lib.rs` (configurable timeouts, logging), `instant/scripts/patch-refresh-runtime.js` (HMR guard), `instant/package.json` (postinstall hook).

### Notes
- The `patch-refresh-runtime.js` is a postinstall convenience patch to avoid editing `node_modules` manually. For an upstream fix, we should open an issue/PR with `@vitejs/plugin-react` including a small reproducer.
- Next: add automated tests for the Windows clipboard flow and a verification script for the runtime patch.

### How to apply locally
```bash
# apply the HMR patch (or it runs during npm install)
node instant/scripts/patch-refresh-runtime.js

# run frontend dev server
cd instant
npm install
npm run dev

# to run the Tauri app (requires Rust + Tauri toolchain)
cd instant
npm run tauri
```

### Environment overrides
- `INSTANT_SELECTED_TEXT_COPY_TIMEOUT_MS` — override clipboard wait timeout (ms). Default: 3000
- `INSTANT_SHORTCUT_KEYS_RELEASE_TIMEOUT_MS` — override shortcut release wait (ms). Default: 1200

### Testing guidance
- Verify HMR no longer crashes when exporting arrow-function components as defaults.
- On Windows, test selected-text capture against slow target apps (Word, Adobe Reader) and simulate long copy durations by adding artificial delay in tests.
- Ensure clipboard restore succeeds for rich/non-text formats.
