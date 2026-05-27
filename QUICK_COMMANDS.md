# Instant AI Context - Quick Start Commands

## One-Time Setup (Copy & Paste)

```bash
# Create Tauri v2 project with React
npm create tauri-app@latest -- \
  --project-name instant-ai-context \
  --package-manager npm \
  --ui react

# Navigate to project
cd instant-ai-context

# Install global shortcut plugin
npm add @tauri-apps/plugin-global-shortcut

# Install Tauri CLI and bundler
npm install --save-dev @tauri-apps/cli @tauri-apps/bundler
```

## Development Commands

```bash
# Start dev server with hot reload
npm run tauri:dev

# Test global shortcut (Ctrl+Shift+Space)
# Application will be hidden on startup
# Press Ctrl+Shift+Space to show overlay
```

## Production Build Commands

```bash
# Build for current platform
npm run tauri:build

# Build for specific platforms
npm run tauri:build:windows     # Windows MSI + NSIS
npm run tauri:build:macos       # macOS universal (Intel + Apple Silicon)
npm run tauri:build:linux       # Linux DEB + AppImage

# Build all platforms
npm run tauri:build:all
```

## Build Output Locations

```
Release binaries:
- Windows: src-tauri/target/release/bundle/msi/ (installer)
           src-tauri/target/release/bundle/nsis/ (portable)
- macOS: src-tauri/target/release/bundle/dmg/ (disk image)
- Linux: src-tauri/target/release/bundle/deb/ (Debian package)
```

## Binary Size Verification

```bash
# After build, check binary size
du -sh src-tauri/target/release/bundle/*/Instant\ AI\ Context*

# Expected: 15-25MB
```

## Troubleshooting

```bash
# Check global shortcut registration
# On macOS: System Preferences > Security & Privacy > Accessibility

# View detailed build logs
npm run tauri:build -- --verbose

# Clean rebuild (if issues occur)
cargo clean
npm run tauri:build
```

## File Structure After Init

```
instant-ai-context/
├── src/
│   ├── App.tsx (React overlay component)
│   ├── App.css (styling)
│   └── main.tsx
├── src-tauri/
│   ├── src/
│   │   └── main.rs (global shortcut handler)
│   ├── capabilities/
│   │   └── default.json (security config)
│   ├── tauri.conf.json (window config)
│   └── Cargo.toml (Rust dependencies + release profile)
├── package.json (npm scripts)
└── tsconfig.json
```

## Key Features Implemented

✅ Headless startup (hidden on boot)
✅ Global shortcut listener (Ctrl+Shift+Space)
✅ Borderless, transparent, always-on-top overlay
✅ Production Rust code (no unwrap/panic)
✅ Ultra-optimized binary size
✅ Cross-platform (Windows, macOS, Linux)
✅ Security capabilities system
✅ Error handling with Result types
