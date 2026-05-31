# Instant AI Context - Architecture & File Reference

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Operating System                         │
│  (Windows 10+ | macOS 10.13+ | Linux Debian-based)         │
└────────────────────┬────────────────────────────────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
        ▼                         ▼
   ┌─────────────┐         ┌──────────────┐
   │   Global    │         │   Overlay    │
   │  Shortcut   │────────▶│   Window     │
   │  Listener   │         │  (Hidden)    │
   └─────────────┘         └──────────────┘
   (Ctrl+Alt+Space primary)      │
        │                         │
        │                    Press hotkey to show
        │                         │
        ▼                         ▼
   ┌──────────────────────────────────────┐
   │   Tauri Core Runtime (Rust)          │
   │  ├─ Global Shortcut Plugin           │
   │  ├─ Window Manager                   │
   │  ├─ Event System                     │
   │  └─ IPC Bridge (Rust ↔ JS)           │
   └──────────────────────────────────────┘
           │                    ▲
           │ invoke commands    │ event listeners
           ▼                    │
   ┌──────────────────────────────────────┐
   │   React Frontend (TypeScript)        │
   │  ├─ App.tsx (Overlay UI)             │
   │  ├─ Event Handlers                   │
   │  └─ Tauri API Integration            │
   └──────────────────────────────────────┘
           │
           ▼
   ┌──────────────────────────────────────┐
   │   Vite Dev Server (Development)      │
   │   or Static Assets (Production)      │
   └──────────────────────────────────────┘
```

---

## File Structure & Purposes

### Configuration Files

| File | Purpose | Priority |
|------|---------|----------|
| `src-tauri/Cargo.toml` | Rust dependencies + release optimizations | 🔴 Critical |
| `src-tauri/tauri.conf.json` | Tauri configuration (window, bundle, build) | 🔴 Critical |
| `src-tauri/capabilities/default.json` | Security permissions for all features | 🔴 Critical |
| `package.json` | NPM scripts and frontend dependencies | 🔴 Critical |
| `.env.example` | Environment variables template | 🟡 Important |
| `vite.config.ts` | Vite bundler configuration | 🟡 Important |
| `tsconfig.json` | TypeScript compiler options | 🟡 Important |

### Source Code Files

| File | Purpose | Priority |
|------|---------|----------|
| `src-tauri/src/main.rs` | Tauri app entry, global shortcut setup | 🔴 Critical |
| `src-tauri/src/lib.rs` | Tauri commands and business logic | 🔴 Critical |
| `src/App.tsx` | React overlay UI component | 🔴 Critical |
| `src/App.css` | Overlay styling (ultra-lightweight) | 🟡 Important |
| `src/main.tsx` | React entry point | 🟡 Important |

### Documentation Files

| File | Purpose |
|------|---------|
| `SETUP_GUIDE.md` | Comprehensive setup and build instructions |
| `QUICK_COMMANDS.md` | Quick reference for terminal commands |
| `DEPLOYMENT_CHECKLIST.md` | Pre-deployment verification steps |
| `ARCHITECTURE.md` | This file - system architecture reference |

---

## Production Release Profile Explanation

### Cargo.toml Release Settings

```toml
[profile.release]
opt-level = "z"           # Optimize for size (smallest binary)
lto = true                # Link-Time Optimization across entire crate
codegen-units = 1         # Single code generation unit (enables LTO)
strip = true              # Strip debug symbols
panic = "abort"           # Smaller panic handler than unwind
split-debuginfo = "packed"# Pack debug info efficiently
```

**Result**: Binary reduced from ~50MB to 15-25MB without compromising performance

---

## Global Shortcut Flow Diagram

```
User presses Ctrl+Alt+Space
        │
        ▼
┌─────────────────────────┐
│ OS Event → Global       │
│ Shortcut Plugin (Tauri) │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────────┐
│ Plugin emits "shortcut"     │
│ event to Rust handler       │
└────────────┬────────────────┘
             │
             ▼
┌─────────────────────────────┐
│ main.rs: toggle_overlay()   │
│ - Check window visibility   │
│ - Hide or Show overlay      │
│ - Center on screen          │
│ - Set focus                 │
└────────────┬────────────────┘
             │
             ▼
┌─────────────────────────────┐
│ Tauri IPC → React via       │
│ invoke() command            │
└────────────┬────────────────┘
             │
             ▼
┌─────────────────────────────┐
│ Window animates in/out      │
│ (CSS transitions)           │
└─────────────────────────────┘
```

---

## Key Design Decisions

### 1. Borderless Window
**Why**: Seamless overlay appearance without OS window chrome
**Trade-off**: Manual close button required

### 2. Always-on-Top
**Why**: Ensures overlay stays visible above all applications
**Risk**: Can be annoying if not properly managed (ESC closes)

### 3. Transparent Background
**Why**: Blends with user's desktop environment
**Implementation**: CSS `backdrop-filter: blur()` for glassmorphism effect

### 4. Headless Startup
**Why**: No intrusive window on boot, responds only to hotkey
**Benefit**: Zero distraction, always available

### 5. Backend-Only AI In Production
**Why**: Public builds must not expose Gemini/OpenAI API keys inside the desktop app.
**How**: Production mode requires the Instant backend and a saved login session. Local/BYOK Gemini fallback is allowed only for development or private testing.

### 6. Single-Purpose Plugin
**Why**: Tauri v2 plugin system is modular; only `global-shortcut` added
**Benefit**: Minimal attack surface, maximum performance

---

## Error Handling Strategy

### Rust (Production-Ready)

```rust
// ❌ AVOID: Crashes on error
let window = windows.get("overlay").unwrap();

// ✅ CORRECT: Propagate errors gracefully
let window = windows
    .iter()
    .find(|(_, w)| w.label() == "overlay")
    .ok_or("Window not found")?;
```

### React (Type-Safe)

```tsx
// ❌ AVOID: Assumes success
await invoke("toggle_overlay");

// ✅ CORRECT: Handle errors
try {
  await invoke("toggle_overlay");
} catch (error) {
  console.error("Error toggling overlay:", error);
  setGreetMsg({ message: "Error: Failed to toggle overlay" });
}
```

---

## Performance Targets

| Metric | Target | Typical |
|--------|--------|---------|
| Startup Time | <500ms | 200-300ms |
| Shortcut Latency | <50ms | 20-40ms |
| Memory (Idle) | <30MB | 15-20MB |
| CPU (Idle) | <0.1% | 0.02-0.05% |
| Binary Size | <30MB | 18-25MB |

---

## Security Model

### Capabilities-Based Access Control
Tauri v2 requires explicit permission declaration for every feature:
- Window management: `core:window:*`
- Global shortcuts: `global-shortcut:*`
- Filesystem: `core:fs:*` (not enabled by default)
- Network: Webview network access is restricted by CSP. AI network calls are made from Rust through the Instant backend in production mode.

### Threat Mitigation
- CSP prevents inline script execution
- IPC validates all command calls
- No eval() or dynamic code execution
- Permissions are granular and auditable
- Production desktop builds do not require user-managed Gemini/OpenAI API keys

---

## Deployment Platforms

### Supported Platforms
- Windows 10+ (x86_64 only)
- macOS 10.13+ (Intel + Apple Silicon)
- Linux (glibc 2.29+, Debian-based)

### Installation Methods
- Windows: MSI (installer) or NSIS (portable)
- macOS: DMG (disk image) or direct executable
- Linux: DEB package or AppImage

### Update Strategy
Currently: Manual downloads
Future: Delta updates with `tauri-updater` plugin (if needed)

---

## Development vs Production

| Aspect | Development | Production |
|--------|-------------|-----------|
| Build Time | ~30-60s | ~2-5min |
| Binary Size | ~100MB+ | ~20MB |
| Optimizations | Off | LTO, strip, opt-z |
| Debugging | Full symbols | Stripped |
| Hot Reload | Yes (Vite) | No |
| Error Details | Verbose | User-friendly |

---

## Next Steps for Enhancement

Phase 2 considerations:
- [ ] Add clipboard monitoring integration
- [ ] Implement persistent settings storage
- [ ] Add system tray menu with quick actions
- [ ] Create update mechanism
- [ ] Add telemetry for usage tracking (privacy-respecting)
- [ ] Implement multi-display awareness
- [ ] Add custom keybinding configuration UI
