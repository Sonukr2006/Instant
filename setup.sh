#!/bin/bash
# Instant AI Context - Automated Setup Script
# This script initializes a production-ready Tauri v2 + React + Rust application

set -e  # Exit on error

echo "🚀 Instant AI Context - Setup Wizard"
echo "======================================"

# Step 1: Create Tauri app
echo "📦 Creating Tauri v2 project..."
npm create tauri-app@latest -- \
  --project-name instant-ai-context \
  --package-manager npm \
  --ui react

cd instant-ai-context

# Step 2: Install dependencies
echo "📥 Installing global-shortcut plugin..."
npm add @tauri-apps/plugin-global-shortcut
npm install --save-dev @tauri-apps/cli @tauri-apps/bundler

# Step 3: Create production configurations
echo "⚙️  Creating production configurations..."

# Cargo.toml
cat > src-tauri/Cargo.toml << 'EOF'
[package]
name = "instant-ai-context"
version = "0.1.0"
description = "Ultra-lightweight desktop AI context utility"
authors = ["Your Name"]
license = "MIT"
edition = "2021"
rust-version = "1.70"

[build-dependencies]
tauri-build = { version = "2.0", features = [] }

[dependencies]
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
tauri = { version = "2.0", features = [
    "macos-private-api",
    "clipboard-manager",
    "fs-all",
    "window-all",
] }
tauri-plugin-global-shortcut = { version = "2.0", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
log = "0.4"
env_logger = "0.10"

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
split-debuginfo = "packed"

[profile.dev]
opt-level = 0

[profile.release.package."*"]
opt-level = "z"
EOF

# tauri.conf.json
cat > src-tauri/tauri.conf.json << 'EOF'
{
  "productName": "Instant AI Context",
  "version": "0.1.0",
  "identifier": "com.instantai.context",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420/",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "overlay",
        "title": "Instant AI Context",
        "url": "index.html",
        "width": 600,
        "height": 400,
        "x": 0,
        "y": 0,
        "visible": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "resizable": false,
        "fullscreen": false,
        "focus": true,
        "skipTaskbar": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:;"
    }
  },
  "systemTray": {
    "iconPath": "icons/icon.png",
    "iconAsTemplate": true,
    "menuOnLeftClick": false
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "nsis", "dmg", "deb", "app"],
    "identifier": "com.instantai.context",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
EOF

# capabilities/default.json
mkdir -p src-tauri/capabilities
cat > src-tauri/capabilities/default.json << 'EOF'
{
  "$schema": "../../../node_modules/@tauri-apps/cli/schema.json",
  "identifier": "default",
  "description": "Default capability set for Instant AI Context",
  "windows": ["main", "overlay"],
  "webApi": {
    "all": false,
    "assetScope": {
      "allow": ["$APP/", "asset:///**"]
    }
  },
  "permissions": [
    "core:app:allow-app-hide",
    "core:app:allow-app-show",
    "core:app:allow-set-focus",
    "core:app:allow-version",
    "core:event:allow-emit",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "core:path:allow-resolve",
    "core:path:allow-normalize",
    "core:path:allow-join",
    "core:path:allow-dirname",
    "core:path:allow-basename",
    "core:path:allow-extname",
    "core:process:allow-exit",
    "core:window:allow-set-always-on-top",
    "core:window:allow-set-decorations",
    "core:window:allow-center",
    "core:window:allow-close",
    "core:window:allow-hide",
    "core:window:allow-show",
    "core:window:allow-set-focus",
    "core:window:allow-set-resizable",
    "core:window:allow-set-size",
    "core:window:allow-set-position",
    "core:window:allow-set-title",
    "core:window:allow-set-skip-taskbar",
    "core:window:allow-set-visible",
    "core:window:allow-get-position",
    "core:window:allow-get-size",
    "core:window:allow-get-current-window",
    "core:window:allow-get-all-windows",
    "core:window:allow-set-content-protected",
    "global-shortcut:allow-register",
    "global-shortcut:allow-is-registered",
    "global-shortcut:allow-unregister",
    "global-shortcut:allow-unregister-all"
  ]
}
EOF

# src-tauri/src/main.rs
cat > src-tauri/src/main.rs << 'EOF'
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Manager, PhysicalPosition};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn toggle_overlay(app: tauri::AppHandle) -> Result<(), String> {
    let windows = app.webview_windows();

    for (_, window) in windows.iter() {
        if window.label() == "overlay" {
            let is_visible = window
                .is_visible()
                .map_err(|e| format!("Failed to check visibility: {}", e))?;

            if is_visible {
                window
                    .hide()
                    .map_err(|e| format!("Failed to hide window: {}", e))?;
            } else {
                window
                    .show()
                    .map_err(|e| format!("Failed to show window: {}", e))?;
                window
                    .set_focus()
                    .map_err(|e| format!("Failed to set focus: {}", e))?;

                if let Ok(monitor) = window.current_monitor() {
                    if let Some(monitor_data) = monitor {
                        let x = (monitor_data.size.width as i32 - 600) / 2 + monitor_data.position.x;
                        let y = (monitor_data.size.height as i32 - 400) / 2 + monitor_data.position.y;
                        let _ = window.set_position(tauri::Position::Physical(PhysicalPosition::new(x, y)));
                    }
                }
            }
            return Ok(());
        }
    }

    Err("Overlay window not found".to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            let global_shortcut = app.handle().global_shortcut();

            match global_shortcut.register("ctrl+shift+space", move || {
                let app = app_handle.clone();
                if let Err(e) = toggle_overlay(app) {
                    eprintln!("Error toggling overlay: {}", e);
                }
            }) {
                Ok(_) => println!("Global shortcut registered successfully"),
                Err(e) => eprintln!("Failed to register global shortcut: {}", e),
            }

            Ok(())
        })
        .on_window_event(|_window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![greet, toggle_overlay])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
EOF

# src/App.tsx
cat > src/App.tsx << 'EOF'
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface GreetMessage {
  message: string;
}

function App() {
  const [greetMsg, setGreetMsg] = useState<GreetMessage | null>(null);
  const [name, setName] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  async function greet() {
    setIsLoading(true);
    try {
      const result = await invoke<string>("greet", { name: name || "User" });
      setGreetMsg({ message: result });
    } catch (error) {
      console.error("Error calling greet:", error);
      setGreetMsg({ message: "Error: Failed to greet" });
    } finally {
      setIsLoading(false);
    }
  }

  async function closeOverlay() {
    try {
      await invoke("toggle_overlay");
    } catch (error) {
      console.error("Error closing overlay:", error);
    }
  }

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeOverlay();
      }
    };

    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, []);

  return (
    <div className="overlay-container">
      <div className="overlay-header">
        <h1>Instant AI Context</h1>
        <button className="close-btn" onClick={closeOverlay}>✕</button>
      </div>

      <div className="overlay-content">
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
          onKeyPress={(e) => { if (e.key === "Enter") greet(); }}
        />
        <button onClick={greet} disabled={isLoading}>
          {isLoading ? "Loading..." : "Greet"}
        </button>

        {greetMsg && <p className="greet-message">{greetMsg.message}</p>}
      </div>

      <div className="overlay-footer">
        <p>Press Ctrl+Shift+Space to toggle • ESC to close</p>
      </div>
    </div>
  );
}

export default App;
EOF

# src/App.css
cat > src/App.css << 'EOF'
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  background: transparent;
  color: #333;
}

.overlay-container {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: rgba(255, 255, 255, 0.95);
  backdrop-filter: blur(10px);
  border-radius: 12px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.3);
  overflow: hidden;
}

.overlay-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
}

.overlay-header h1 {
  font-size: 18px;
  font-weight: 600;
}

.close-btn {
  background: rgba(255, 255, 255, 0.2);
  border: none;
  color: white;
  width: 32px;
  height: 32px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 18px;
  transition: background 0.2s;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.3);
}

.overlay-content {
  flex: 1;
  padding: 24px 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
}

#greet-input {
  padding: 10px 12px;
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 6px;
  font-size: 14px;
}

#greet-input:focus {
  outline: none;
  border-color: #667eea;
  box-shadow: 0 0 0 2px rgba(102, 126, 234, 0.1);
}

button {
  padding: 10px 16px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-weight: 500;
  font-size: 14px;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.greet-message {
  padding: 12px;
  background: rgba(102, 126, 234, 0.1);
  border-left: 3px solid #667eea;
  border-radius: 4px;
  font-size: 14px;
}

.overlay-footer {
  padding: 12px 20px;
  border-top: 1px solid rgba(0, 0, 0, 0.05);
  background: rgba(0, 0, 0, 0.02);
  font-size: 12px;
  color: rgba(0, 0, 0, 0.6);
  text-align: center;
}
EOF

echo "✅ Setup complete!"
echo ""
echo "📖 Next steps:"
echo "   1. npm run tauri:dev     (Start development server)"
echo "   2. Press Ctrl+Shift+Space to toggle overlay"
echo ""
echo "🏗️  To build for production:"
echo "   npm run tauri:build      (Build for current platform)"
echo "   npm run tauri:build:all  (Build for all platforms)"
echo ""
