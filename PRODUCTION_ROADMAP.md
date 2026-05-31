# Instant AI - Production Launch Roadmap

This document is the source of truth for taking Instant AI from prototype/MVP to a public production launch.

Goal: ship a public desktop product that lets users select or copy text anywhere, open an overlay instantly, and get AI-powered explanations without switching to a browser.

## Product Target

Instant AI is a desktop learning and productivity assistant.

Primary user:
- Students
- Developers
- Researchers
- Professionals reading PDFs, docs, articles, papers, and study material

Primary promise:
- Highlight or copy text
- Open Instant AI quickly
- Ask/explain without browser switching
- Keep the user's reading flow uninterrupted

## Production Architecture

```text
Desktop App
  |
  |-- Lightweight tray/listener layer
  |     |-- Windows global shortcut
  |     |-- Linux tray-click workflow
  |     |-- Low idle CPU and memory
  |
  |-- Overlay UI
  |     |-- Captured selected/clipboard text
  |     |-- Prompt editor
  |     |-- Streaming AI response
  |     |-- Retry/cancel/copy actions
  |
  |-- Secure auth storage
        |-- Windows Credential Manager
        |-- macOS Keychain
        |-- Linux Secret Service/KWallet or documented fallback

Backend API
  |
  |-- Authentication/session validation
  |-- User and subscription state
  |-- Quota/rate limiting
  |-- AI provider abstraction
  |     |-- Gemini
  |     |-- OpenAI-compatible providers
  |
  |-- Streaming response endpoint
  |-- Observability, logging, metrics, alerts

Infrastructure
  |
  |-- Database: users, sessions, plans, billing state
  |-- Redis/Postgres quota counters
  |-- Secret manager for provider API keys
  |-- CI/CD
  |-- Signed desktop builds
  |-- Rollback and release strategy
```

## Production Rules

- Production desktop builds must not contain Gemini/OpenAI API keys.
- Production AI requests must go through the backend.
- Selected/copied user text must never be logged.
- Tokens and API keys must never be logged.
- Quota must not be in-memory for public launch.
- Auth must not depend on manually minted dev JWTs.
- Linux Wayland must use clipboard/tray fallback, not unrestricted selected-text capture.
- Windows selected-text capture is a core launch feature.
- Documentation must match actual supported behavior.

## Phase 0 - Current Status

Status: Incomplete

Already working:
- Tauri desktop app foundation
- Overlay UI
- Tray support
- Clipboard read command
- Backend proxy foundation
- JWT validation improvements
- Quota refund on AI failure
- Gemini response parsing
- Secure token storage foundation
- Basic Rust tests and clippy checks

Still not production-ready:
- No Windows selected-text capture
- No true lightweight listener/main separation
- Local Gemini fallback remains only for development/private builds
- In-memory quota
- Manual dev-token auth
- Gemini-specific backend
- No streaming response
- No production monitoring
- Stale architecture docs
- No signed public build pipeline

## Phase 1 - Production Mode Cleanup

Status: Complete

Purpose:
Make the app safe for public production builds.

Steps:
1. Add explicit app mode.
   - `development`
   - `production`

2. In production mode:
   - Disable local Gemini fallback.
   - Require backend API URL.
   - Require authenticated session.
   - Hide/remove user-facing Gemini API key setup.

3. Keep local/BYOK Gemini only for development or private builds.

Completed implementation:
- Added explicit `INSTANT_APP_MODE`.
- Debug builds default to `development`.
- Release builds force `production` and ignore config/env attempts to downgrade to development mode.
- Production mode rejects local Gemini fallback.
- Production mode requires backend configuration and login session.
- Updated app env template and production docs.

Files likely involved:
- `instant/.env.example`
- `instant/src-tauri/src/lib.rs`
- `instant/src/services/aiService.ts`
- `backend/README.md`
- `ARCHITECTURE.md`

Acceptance criteria:
- Production app cannot call Gemini directly.
- Production app shows clear error if backend/auth is missing.
- No production docs tell users to set a Gemini key locally.

Test cases:
- Production mode without backend URL fails clearly.
- Production mode without token fails clearly.
- Development mode local Gemini fallback still works if intentionally enabled.
- No API key is bundled in production app.

## Phase 2 - Windows Selected-Text Capture

Status: In Progress

Purpose:
Deliver the core product workflow for Windows.

Required workflow:
1. User selects text anywhere.
2. User presses global shortcut.
3. App captures selected text.
4. Overlay opens.
5. Captured text appears in prompt box.

Implementation approach:
- Register Windows global shortcut.
- Preserve current clipboard content before capture.
- Simulate `Ctrl+C`.
- Wait briefly for clipboard update.
- Read clipboard text.
- Restore previous clipboard content after capture.
- Open/focus overlay.
- Emit captured text to frontend.

Current implementation:
- Windows shortcut now attempts selected-text capture before opening the overlay.
- Windows primary shortcut is `Ctrl+Alt+Space`; legacy `Ctrl+Shift+Space` remains temporarily supported but conflicts with Word/Office and browser inspect shortcuts.
- Windows capture uses native `SendInput` for `Ctrl+C`.
- Clipboard sequence number is checked to detect whether copy actually happened.
- Clipboard content is snapshotted and restored for safely readable Windows formats.
- Selected-text capture is skipped instead of overwriting clipboard data when unsupported formats cannot be safely backed up.
- Clipboard backup is capped by format count and byte size to avoid shortcut-time memory spikes.
- If selected-text capture fails, the app falls back to clipboard text.
- React now listens for a `context-captured` event instead of racing against window-focus clipboard reads.
- The app no longer reads clipboard content while hidden at startup.
- Focus events no longer clear the response or prompt.
- Rapid Windows shortcut presses are guarded to avoid overlapping capture attempts.
- Shortcut capture waits for modifier keys to be released before sending `Ctrl+C`, reducing app shortcut conflicts.
- Windows GitHub Actions CI has been added to compile the native Windows path.

Pending validation:
- Confirm the Windows CI workflow passes on GitHub.
- Runtime test on an actual Windows machine.
- Verify clipboard restore behavior with rich/non-text clipboard formats.
- Verify shortcut behavior across Notepad, browsers, PDF readers, Office apps, and Electron apps.
- Replace hardcoded shortcuts with user-configurable shortcuts before public production launch.

Files likely involved:
- `instant/src-tauri/src/lib.rs`
- `instant/src/App.tsx`
- `instant/src-tauri/Cargo.toml`
- `instant/src-tauri/tauri.conf.json`

Acceptance criteria:
- Shortcut captures selected text from common apps.
- Overlay opens with captured text.
- Clipboard is not permanently destroyed in normal cases.
- If selected-text capture fails, app falls back to existing clipboard text with clear UX.

Test cases:
- Notepad selected text.
- Browser selected text.
- PDF reader selected text.
- No text selected.
- Clipboard contains image.
- Clipboard contains previous text.
- Slow app copy response.
- Rapid shortcut presses.

## Phase 3 - Production Privacy and Failure Hardening

Status: In Progress

Purpose:
Make capture, AI requests, and error handling safe enough for public users.

Current implementation:
- Selected/copied text is not intentionally written to desktop or backend logs.
- Production desktop builds require backend/auth and do not expose Gemini keys.
- Windows capture uses UI Automation before clipboard fallback when possible.
- Clipboard fallback avoids unsupported clipboard formats and restores backed-up content.
- Frontend no longer logs raw error objects in production.
- Gemini/backend transport and HTTP failures use sanitized user-facing messages.
- First AI request requires explicit privacy confirmation before prompt text is sent.

Remaining work:
- Add configurable shortcuts instead of hardcoded shortcuts.
- Add opt-in crash reporting with strict redaction.
- Add structured backend request IDs without logging prompt bodies.
- Add persistent quota storage before public launch.

Files involved:
- `instant/src-tauri/src/lib.rs`
- `instant/src/App.tsx`
- `instant/src/services/aiService.ts`
- `backend/src/main.rs`
- `DEPLOYMENT_CHECKLIST.md`

Acceptance criteria:
- Prompt context, selected text, clipboard text, tokens, and provider API keys do not appear in logs.
- Network failures and provider failures show clean, actionable messages.
- Public builds fail closed when backend/auth is missing.
- Clipboard fallback never silently destroys existing clipboard content.

Test cases:
- AI provider timeout.
- AI provider HTTP 401/403.
- AI provider HTTP 429.
- AI provider HTTP 500.
- Backend missing auth token.
- Clipboard with non-text data.
- Production frontend error path.
- First AI request before privacy confirmation.

## Phase 4 - Linux Wayland-Safe Workflow

Status: Partially Complete

Purpose:
Support Linux without violating Wayland security restrictions.

Current state:
- Tray icon exists.
- Clipboard read exists.

Required workflow:
1. User manually copies text.
2. User clicks tray icon.
3. Overlay opens.
4. Clipboard text appears in prompt box.

Files likely involved:
- `instant/src-tauri/src/lib.rs`
- `instant/src/App.tsx`
- `instant/src-tauri/tauri.conf.json`
- `ARCHITECTURE.md`

Acceptance criteria:
- Linux docs clearly explain clipboard-first workflow.
- No unsupported Wayland selected-text promise.
- Tray click reliably opens overlay.
- Empty clipboard gives useful message.

Test cases:
- GNOME Wayland.
- KDE Wayland.
- X11 session.
- Clipboard empty.
- Clipboard has non-text data.

## Phase 5 - Real Authentication

Status: Incomplete

Purpose:
Replace dev JWT minting with public-user authentication.

Required:
- User accounts.
- Session creation.
- Session revocation.
- Token refresh or short-lived access tokens.
- Server-side user status check.
- Server-side plan/subscription lookup.

Current risk:
- `mint_token.rs` is useful for development only.
- Backend trusts JWT `plan`.

Files likely involved:
- `backend/src/main.rs`
- `backend/src/bin/mint_token.rs`
- `backend/Cargo.toml`
- `instant/src-tauri/src/auth.rs`
- `instant/src/services/authService.ts`
- New backend auth modules

Acceptance criteria:
- Public users can log in without manually pasted dev tokens.
- Revoked/expired sessions stop working.
- Plan is loaded from backend database, not trusted from client token only.

Test cases:
- Login success.
- Invalid credentials/session.
- Expired session.
- Revoked session.
- Downgraded subscription.
- Deleted/banned user.

## Phase 5 - Persistent Quota and Rate Limiting

Status: Incomplete

Purpose:
Protect cost and prevent abuse.

Required:
- Redis or Postgres quota counters.
- Per-user daily limits.
- Per-IP rate limits.
- Global concurrency limits.
- Gemini/OpenAI provider rate-limit handling.

Current risk:
- Backend quota is in-memory.
- Restart resets quota.
- Multiple backend instances do not share quota.

Files likely involved:
- `backend/src/main.rs`
- `backend/Cargo.toml`
- New quota/rate-limit module
- Infrastructure config

Acceptance criteria:
- Quota survives restart.
- Multiple backend instances share quota.
- Abuse does not create unlimited provider cost.
- Upstream AI failures do not incorrectly consume quota.

Test cases:
- Concurrent requests from one user.
- Backend restart.
- Multiple backend instances.
- AI timeout.
- AI 429/5xx response.
- Daily reset.

## Phase 6 - AI Provider Abstraction

Status: Incomplete

Purpose:
Support Gemini, OpenAI, and OpenAI-compatible providers cleanly.

Required:
- Provider trait/interface.
- Provider-specific request/response mapping.
- Shared internal AI request model.
- Shared error model.
- Model/provider config.

Current risk:
- Backend and client fallback are Gemini-specific.

Files likely involved:
- `backend/src/main.rs`
- New `backend/src/providers/` module
- `backend/.env.example`
- `backend/README.md`

Acceptance criteria:
- Backend route is provider-neutral.
- Gemini can be swapped without frontend changes.
- Provider errors map to stable app errors.

Test cases:
- Gemini success.
- Gemini blocked response.
- OpenAI-compatible success.
- Provider timeout.
- Provider rate limit.
- Provider malformed response.

## Phase 7 - Streaming Response and Cancel

Status: Incomplete

Purpose:
Make the product feel instant and responsive.

Required:
- Streaming backend endpoint.
- Frontend incremental rendering.
- Cancel active request.
- Retry failed request.

Files likely involved:
- `backend/src/main.rs`
- `instant/src-tauri/src/lib.rs`
- `instant/src/App.tsx`
- `instant/src/components/ResponseArea.tsx`
- `instant/src/services/aiService.ts`

Acceptance criteria:
- User sees response start quickly.
- Closing overlay or pressing cancel stops the request.
- Retry does not duplicate old state.

Test cases:
- Slow network.
- Long response.
- Cancel mid-stream.
- Retry after timeout.
- Hide/show overlay during stream.

## Phase 8 - Privacy and Security Hardening

Status: Incomplete

Purpose:
Make the app safe for public trust.

Required:
- No prompt logging.
- No token/API key logging.
- Privacy policy.
- Clear user consent for sending selected text to AI.
- CSP review.
- Dependency audit.
- Secret manager for backend.

Files likely involved:
- `backend/src/main.rs`
- `instant/src-tauri/tauri.conf.json`
- `instant/src-tauri/capabilities/default.json`
- `README.md`
- New `PRIVACY.md`

Acceptance criteria:
- Logs do not contain selected text.
- Logs do not contain auth tokens.
- Backend secrets are not stored in repo.
- Public privacy policy exists.

Test cases:
- Backend error with prompt does not log prompt.
- Token parse failure does not log token.
- Provider error does not expose API key.
- Security audit passes.

## Phase 9 - Observability and Operations

Status: Incomplete

Purpose:
Debug real production issues quickly.

Required metrics:
- Request count.
- Latency p50/p95/p99.
- Auth failures.
- Quota rejects.
- AI provider status codes.
- AI provider latency.
- App crash/error reports.
- Cost per user/day.

Files likely involved:
- `backend/src/main.rs`
- `backend/Cargo.toml`
- Deployment/infrastructure config
- Desktop logging config

Acceptance criteria:
- Every request has correlation ID.
- Provider failures are visible.
- Alerts exist for high 5xx, high latency, cost spike, auth failure spike.

Test cases:
- Simulated Gemini outage triggers alerts.
- High auth failure rate visible.
- Request ID appears across logs.
- No private prompt data appears in logs.

## Phase 10 - Production Packaging and Release

Status: Incomplete

Purpose:
Ship safely to public users.

Required:
- Windows signed installer.
- Versioned backend deployment.
- CI/CD pipeline.
- Release notes.
- Rollback plan.
- Update strategy.

Files likely involved:
- `instant/src-tauri/tauri.conf.json`
- `instant/package.json`
- `.github/workflows/*`
- `DEPLOYMENT_CHECKLIST.md`
- `README.md`

Acceptance criteria:
- Clean clone builds in CI.
- Windows installer is signed.
- Backend can roll back independently.
- Release artifacts are versioned.

Test cases:
- Fresh install.
- Upgrade install.
- Uninstall.
- Backend rollback.
- Expired client version behavior.

## Launch Readiness Checklist

- [x] Production mode disables local AI keys.
- [ ] Windows shortcut captures selected text.
- [ ] Linux tray/clipboard fallback is documented and tested.
- [ ] Real auth replaces manual dev JWTs.
- [ ] Plan/subscription state is checked server-side.
- [ ] Quota is persisted in Redis/Postgres.
- [ ] Rate limiting and concurrency limiting exist.
- [ ] AI provider abstraction exists.
- [ ] Streaming response works.
- [ ] Cancel/retry works.
- [ ] No selected text is logged.
- [ ] Monitoring and alerts exist.
- [ ] CI runs tests, clippy, build, and audit.
- [ ] Windows installer is signed.
- [ ] Privacy policy exists.
- [ ] Docs match actual app behavior.

## Recommended Build Order

1. Production mode cleanup.
2. Windows selected-text capture.
3. Linux clipboard/tray polish.
4. Real auth.
5. Persistent quota/rate limits.
6. Provider abstraction.
7. Streaming/cancel.
8. Privacy/security hardening.
9. Observability.
10. Packaging/signing/public release.

## Files To Update First

Immediate priority:
- `instant/src-tauri/src/lib.rs`
- `instant/src/App.tsx`
- `instant/.env.example`
- `backend/src/main.rs`
- `ARCHITECTURE.md`

Documentation priority:
- `README.md`
- `backend/README.md`
- `DEPLOYMENT_CHECKLIST.md`
- New `PRIVACY.md`

Future architecture split:
- `backend/src/auth/`
- `backend/src/quota/`
- `backend/src/providers/`
- `backend/src/observability/`
