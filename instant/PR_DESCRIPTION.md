Title: Fix HMR runtime crash and make Windows selected-text capture robust

Branch: fix/clipboard-hmr-patches

Summary:
- Add a lightweight postinstall patch script to guard unsafe `type.prototype` access in `@vitejs/plugin-react` HMR runtime and avoid TypeErrors when arrow functions or host/native functions are exported.
- Make Windows selected-text capture timeouts configurable and add logging for timeout conditions to avoid nondeterministic clipboard races and to surface telemetry.

Files changed:
- instant/scripts/patch-refresh-runtime.js  (new)
- instant/package.json (postinstall hook)
- instant/src-tauri/src/lib.rs (configurable timeouts + logging)
- instant/CHANGELOG.md (new)

Testing instructions:
1. Frontend (HMR):
   - Run `node instant/scripts/patch-refresh-runtime.js` to apply patch to `node_modules`.
   - Start `cd instant && npm install && npm run dev` and confirm HMR works when editing an app exported as an arrow function (e.g., `const App = () => {}`) and no runtime TypeError is thrown.

2. Windows selected-text capture (manual):
   - Build and run the app with Tauri on Windows.
   - Try the shortcut on apps with slow copy semantics (Word, Adobe Reader). Observe logs for warnings about timeouts.
   - Test env overrides:
     - `INSTANT_SELECTED_TEXT_COPY_TIMEOUT_MS=5000` (increase copy wait)
     - `INSTANT_SHORTCUT_KEYS_RELEASE_TIMEOUT_MS=2000` (increase shortcut release wait)

3. Restore behavior:
   - Confirm clipboard formats are backed up and restored after capture in apps that copy rich clipboard formats.

Follow-ups (next PR):
- Add automated unit/integration tests for the clipboard capture flow (Windows) and a test that verifies the runtime patch effect.
- Consider upstreaming the HMR guard into `@vitejs/plugin-react` with a reproducible test case.

Reviewer checklist:
- [ ] Confirm HMR runtime patch is safe and minimal.
- [ ] Validate clipboard restore for common rich formats.
- [ ] Verify logging/telemetry entries for timeout events.
- [ ] Approve and merge.
