CoolChatty – Development Roadmap (MVP)
Phase 1 — Foundation

 Verify Tauri boots (tray + settings window)

 Confirm Rust toolchain works (cargo check)

 Confirm Node toolchain works (npm install)

Phase 2 — Backend Core

 Implement deep error handling for audio

 Plug audio.rs into websocket.rs pipeline

 Add retry logic for network failures

 Add logging for failed paste events

Phase 3 — Hotkeys

 Implement configurable hotkeys in settings

 Bind/unbind on settings change

 UI feedback if hotkey is unavailable

Phase 4 — Paste Logic

 Improve text-field detection (macOS + Windows)

 Add fallback clipboard notification

 Add "Paste manually" option in tray

Phase 5 — History

 Finish history UI polish

 Add delete-all

 Add export (CSV or JSON)

Phase 6 — Frontend Polish

 Style Settings UI

 Add inline help tooltips

 Add onboarding screen

Phase 7 — Packaging

 Configure updater endpoint

 Replace placeholder icons

 Produce first signed builds

Phase 8 — Pre-Release QA

 Test: long recordings

 Test: offline mode

 Test: corrupted audio frames

 Test: hotkey conflicts

 Test: Windows IME fields

