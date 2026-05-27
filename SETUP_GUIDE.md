# Instant AI Context - Production Setup & Build Guide

## Prerequisites
- Node.js 18+ or Bun 1.0+
- Rust 1.70+ (install via https://rustup.rs/)
- OS: Windows 10+, macOS 10.13+, or Linux (Debian-based)

---

## Development Setup

### 1. Create Tauri v2 Project
```bash
npm create tauri-app@latest -- \
  --project-name instant-ai-context \
  --package-manager npm \
  --ui react

cd instant-ai-context
```

### 2. Install Global Shortcut Plugin
```bash
npm add @tauri-apps/plugin-global-shortcut
npm install --save-dev @tauri-apps/cli
```

### 3. Copy Configuration Files
Replace these files with production configurations:
- `src-tauri/Cargo.toml` - Production release profile with binary optimization
- `src-tauri/tauri.conf.json` - Headless overlay window configuration
- `src-tauri/src/main.rs` - Global shortcut handler with error handling
- `src-tauri/capabilities/default.json` - Security permissions
- `src/App.tsx` - React overlay component
- `src/App.css` - Ultra-lightweight styling

### 4. Run Development Server
```bash
npm run tauri dev
```

Test the global shortcut:
- Press `Ctrl+Shift+Space` to toggle overlay
- Press `ESC` to close overlay

---

## Production Build & Release

### Build for Release (Ultra-Optimized Binary)

#### Windows
```bash
npm run tauri build -- --target x86_64-pc-windows-msvc
# Output: src-tauri/target/release/bundle/msi/
# Output: src-tauri/target/release/bundle/nsis/
```

#### macOS
```bash
npm run tauri build -- --target universal-apple-darwin
# Output: src-tauri/target/release/bundle/dmg/
# Output: src-tauri/target/release/bundle/app/
```

#### Linux
```bash
npm run tauri build -- --target x86_64-unknown-linux-gnu
# Output: src-tauri/target/release/bundle/deb/
# Output: src-tauri/target/release/bundle/appimage/
```

### Binary Size Optimization Checklist
✅ `lto = true` - Link-time optimization
✅ `opt-level = "z"` - Optimize for size
✅ `codegen-units = 1` - Single codegen unit
✅ `strip = true` - Strip debug symbols
✅ `panic = "abort"` - Smaller panic handler
✅ `split-debuginfo = "packed"` - Packed debug info

**Expected binary size: 15-25MB (Windows), 20-30MB (macOS), 12-18MB (Linux)**

---

## Platform-Specific Notes

### Windows
- Global shortcuts require Windows 10+
- MSI installer for enterprise deployment
- NSIS installer for user-friendly setup

### macOS
- Requires accessibility permissions for global shortcuts
- Prompt appears on first run
- Universal binary supports both Intel and Apple Silicon
- Code signing required for distribution

### Linux
- Global shortcuts work via X11/Wayland
- Ensure `libxcb-randr0-dev` is installed
- Deb package for Debian/Ubuntu systems

---

## Security Best Practices

1. **Capabilities System**: All permissions explicitly declared in `capabilities/default.json`
2. **CSP Headers**: Content Security Policy configured in `tauri.conf.json`
3. **Error Handling**: Production code avoids `unwrap()` and `panic!()`
4. **Window Isolation**: Overlay window is transparent, borderless, and always-on-top
5. **No Network**: This build includes no network dependencies by default

---

## Global Shortcut Troubleshooting

| Issue | Solution |
|-------|----------|
| Shortcut not working | Check OS-level keyboard shortcuts aren't conflicting |
| macOS: "Permission denied" | Grant accessibility permissions in System Preferences |
| Linux: No response | Ensure X11/Wayland is running, check compositor |
| Windows: Delayed | Verify no antivirus is blocking keyboard hooks |

---

## Performance Metrics

- **Startup time**: <500ms
- **Memory footprint**: <30MB
- **CPU idle**: <0.1%
- **Global shortcut latency**: <50ms

---

## Continuous Deployment

### GitHub Actions Example
```yaml
name: Build Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [windows-latest, macos-latest, ubuntu-latest]
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - uses: tauri-apps/tauri-action@v0
```

---

## Migration from Tauri v1 to v2

Key breaking changes:
- Plugin API completely redesigned
- Global shortcut API changed to builder pattern
- Capabilities system is now mandatory
- Window configuration differs significantly

Use the official migration guide: https://tauri.app/en/develop/migration-guide/

---

## Support & Documentation

- Tauri v2 Docs: https://tauri.app/
- Global Shortcut Plugin: https://github.com/tauri-apps/tauri/tree/next/plugins/global-shortcut
- Issue Tracker: https://github.com/tauri-apps/tauri/issues
