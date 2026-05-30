# Instant AI Desktop App

Tauri + React desktop client for Instant AI.

## Production Mode

Public production builds must use the Instant backend. They must not depend on local Gemini/OpenAI API keys.

Production behavior:
- `INSTANT_APP_MODE=production`
- Backend URL is required.
- A saved login session is required.
- Local Gemini fallback is disabled.
- Release builds force production mode even if a local config file tries to set development mode.

## Development Mode

Development/debug builds default to `development` mode and may use local/BYOK Gemini fallback for private testing.

Example `.env`:

```bash
INSTANT_APP_MODE=development
INSTANT_API_BASE_URL=http://127.0.0.1:8080
INSTANT_API_TOKEN=<dev_token>
```

If backend values are not configured in development mode, the app can use `GEMINI_API_KEY` for local testing.

## Commands

```bash
npm run build
npm run tauri
```
