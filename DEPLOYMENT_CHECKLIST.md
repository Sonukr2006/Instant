# Instant AI Context - Production Deployment Checklist

## Pre-Deployment Verification

### Code Quality
- [ ] No `unwrap()` or `panic!()` in production code paths
- [ ] All Rust functions return `Result<T, E>` types
- [ ] Error messages are descriptive and user-friendly
- [ ] Selected/copied text is never written to application logs
- [ ] API keys, bearer tokens, and provider URLs with query secrets are never written to logs
- [ ] Third-party provider error bodies are not shown directly to users
- [ ] Async operations properly handled with `.await`
- [ ] Global shortcut registration wrapped in error handling

### React Frontend
- [ ] All API calls wrapped in try-catch
- [ ] Loading states implemented for async operations
- [ ] Keyboard event listeners cleaned up (ESC to close)
- [ ] Overlay window resizes properly on different displays
- [ ] No console errors in development mode
- [ ] Production frontend does not log raw error objects that may contain sensitive text or tokens

### Configuration
- [ ] Capabilities explicitly defined in `capabilities/default.json`
- [ ] Window config matches production requirements:
  - [ ] `visible: false` (hidden on startup)
  - [ ] `decorations: false` (borderless)
  - [ ] `transparent: true` (transparency enabled)
  - [ ] `alwaysOnTop: false` at rest; app may temporarily raise the overlay only while showing it
  - [ ] `resizable: false` (fixed dimensions)
- [ ] Release profile optimizations enabled in Cargo.toml

### Performance
- [ ] Binary size verified (<30MB for all platforms)
- [ ] Startup time measured (<500ms)
- [ ] Memory footprint tested (<30MB idle)
- [ ] Global shortcut latency acceptable (<100ms)
- [ ] No memory leaks detected during extended use

### Security
- [ ] No credentials or secrets in code
- [ ] User selected text is sent only after an explicit user action
- [ ] Clipboard fallback restores prior clipboard content or fails closed
- [ ] Server-side provider failures return sanitized messages
- [ ] CSP headers properly configured
- [ ] Window isolation verified
- [ ] No network requests to untrusted sources
- [ ] Accessibility permissions properly documented

### Cross-Platform Testing

#### Windows
- [ ] MSI installer creates start menu entry
- [ ] NSIS installer runs without UAC prompts
- [ ] Global shortcut works with screen reader active
- [ ] Taskbar behavior correct (skipTaskbar: true)
- [ ] Overlay appears on correct monitor (multi-monitor)

#### macOS
- [ ] App signed and notarized (for distribution)
- [ ] Universal binary works on Intel and Apple Silicon
- [ ] Accessibility permissions prompt appears on first run
- [ ] DMG installer mounts without issues
- [ ] Overlay respects macOS window layering

#### Linux
- [ ] Deb package installs dependencies
- [ ] AppImage runs on older glibc versions
- [ ] Global shortcut works with GNOME/KDE/other DEs
- [ ] Wayland compatibility tested
- [ ] No missing library dependencies

### Documentation
- [ ] README.md includes installation instructions
- [ ] Shortcut conflicts documented
- [ ] Troubleshooting guide created
- [ ] Build instructions are accurate
- [ ] Release notes prepared for v0.1.0

### CI/CD Pipeline
- [ ] GitHub Actions workflow created (if applicable)
- [ ] All platforms build successfully in CI
- [ ] Automated testing enabled
- [ ] Release artifacts uploaded automatically
- [ ] Changelog auto-generated from git tags

---

## Deployment Steps

### 1. Final Build
```bash
npm run tauri:build:all
```

### 2. Verify Binary Sizes
```bash
du -sh src-tauri/target/release/bundle/*/
```

### 3. Test Installer on Each Platform
```bash
# Windows: Run .msi and .exe from NSIS folder
# macOS: Mount .dmg and verify app works
# Linux: dpkg -i *.deb && run application
```

### 4. Code Sign & Notarize (Production)
- Windows: Obtain EV code signing certificate
- macOS: Use Apple Developer Program certificate
- Linux: Create GPG signature for releases

### 5. Create GitHub Release
```bash
git tag v0.1.0
git push origin v0.1.0
# Upload binaries to GitHub Releases
```

### 6. Publish to Distribution Channels
- Windows: Microsoft Store (optional)
- macOS: Mac App Store (optional)
- Linux: Flathub, Snap Store (optional)

---

## Post-Deployment Monitoring

### Track Metrics
- Download statistics
- Error reporting (implement crash reporting if needed)
- User feedback channels
- Performance data from telemetry

### Security Updates
- Monitor Rust security advisories: https://rustsec.org/
- Monitor Tauri security advisories
- Update dependencies regularly
- Create patch releases for critical bugs

### Bug Reporting
- GitHub Issues for public bug tracking
- Response time: Critical bugs within 24 hours
- Regression testing before each release

---

## Success Criteria

✅ App launches in <500ms
✅ Global shortcut responds within 50ms
✅ Binary size: Windows <25MB, macOS <30MB, Linux <20MB
✅ Zero crashes in first 1000 installations
✅ 100% of shortcuts work across all platforms
✅ Accessibility permissions handled gracefully
✅ Memory stays below 30MB during idle

---

## Rollback Procedure

If critical issues found post-deployment:

```bash
# Revert to previous tag
git checkout v0.0.1

# Create hotfix branch
git checkout -b hotfix/critical-bug

# Fix, test, rebuild
npm run tauri:build:all

# Tag as patch release
git tag v0.0.2
```
