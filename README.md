# CoolChatty – Cross-Platform Realtime Speech-to-Text (Rust + Tauri)

CoolChatty is a high-performance, cross-platform speech-to-text tool using Tauri (Rust backend + TypeScript frontend).  
It lets the user speak into ANY text field on macOS or Windows using a global hotkey.  
The speech is streamed to the OpenAI Realtime API and converted into instantly usable, cleaned-up text.

If a text field is focused → the text is pasted automatically.  
If not → the text is copied to the clipboard + stored in the built-in history.

CoolChatty is designed for:
- personal productivity
- team workflows
- corporate environments
- future commercial distribution

---

## 🚀 Features
### 🎤 Realtime Speech Recognition
- Low latency (200–400 ms)
- Audio capture with CPAL (Rust)
- WebSocket streaming to OpenAI Realtime API
- Optional LLM cleanup

### ⌨️ Global Hotkey
- Default: Alt + Space
- User-configurable
- Press = start, release = stop

### 📋 Smart Output Logic
- If a text field is active → auto-paste
- Otherwise:
  - copied to clipboard
  - stored in SQLite history

### 📚 History
- SQLite backend
- timestamp, duration, text
- UI for browsing, copying, deleting

### ⚙️ Settings UI
- API key
- Hotkey binding
- Auto-paste toggle
- History toggle
- Model selection

### 🖥️ System Tray App
- Lightweight
- Cross-platform
- Very low RAM usage (~15–30 MB)

---

## 🏗 Architecture Overview
See: `docs/ARCHITECTURE.md`

---

## 📦 Development Setup

### Requirements
- Node.js LTS
- Rust stable
- Tauri CLI: `cargo install tauri-cli`

### Install deps
```bash
npm install
npm run tauri dev
npm run tauri build
```
This produces:

.app (macOS)

.exe / .msix (Windows)

🔒 Security

No audio stored; raw frames are transient

Only outgoing connection: api.openai.com

History and settings stored locally

No clipboard monitoring or file scanning

📜 License

Proprietary (early internal build)
