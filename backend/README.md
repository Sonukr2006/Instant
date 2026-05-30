# Instant AI Context API

Remote Rust backend for public Instant AI Context deployments.

## What It Does

- Authenticates app requests with a bearer JWT.
- Enforces a simple per-user daily request quota.
- Keeps the Gemini API key on the server, not inside the desktop app.
- Proxies `/v1/ai/ask` requests to Gemini.

## Run Locally

```bash
cp .env.example .env
cargo run
```

Required environment values:

- `JWT_SECRET`
- `GEMINI_API_KEY`

## Connect The Tauri App

Start the backend:

```bash
cd backend
cargo run
```

Create a dev JWT for the desktop app:

```bash
cd backend
cargo run --bin mint_token -- dev-user free 30
```

Set these in `instant/.env`:

```bash
INSTANT_APP_MODE=development
INSTANT_API_BASE_URL=http://127.0.0.1:8080
INSTANT_API_TOKEN=<token_from_mint_token>
```

When `INSTANT_API_BASE_URL` is present and the app has a saved token, the Tauri app calls this backend instead of calling Gemini directly. For development you can still use `INSTANT_API_TOKEN`, but the app now also has a small token save/logout UI so users do not need to log in every launch.

For public production builds, set `INSTANT_APP_MODE=production` and require the desktop app to use the backend. Do not ship production builds that depend on a local Gemini/OpenAI API key in the desktop app.

Saved app tokens are stored through the OS credential store where available:

- Windows Credential Manager
- macOS Keychain
- Linux Secret Service/KWallet compatible backend

## Routes

```text
GET  /health
POST /v1/ai/ask
```

`POST /v1/ai/ask` body:

```json
{
  "prompt_context": "Explain this code..."
}
```

The request must include:

```text
Authorization: Bearer <jwt>
```

JWT claims expected:

```json
{
  "sub": "user-id",
  "plan": "free",
  "exp": 1999999999
}
```

## Production Notes

This is a deployable MVP foundation. For multi-server production, replace the in-memory quota map with PostgreSQL or Redis so usage limits are shared across instances.

The `mint_token` binary is a development helper only. Public production auth should issue and revoke user sessions through a real auth flow, then load plan/subscription state server-side.
