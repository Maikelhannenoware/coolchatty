CoolChatty – Architecture
High-Level Diagram
       ┌───────────────────────────────┐
       │           CoolChatty           │
       └───────────────────────────────┘
                       ▲
                       │ Tauri IPC
                       ▼
  ┌─────────────────────────────────────────────┐
  │                Rust Backend                 │
  ├─────────────────────────────────────────────┤
  │ audio.rs       → CPAL microphone capture    │
  │ websocket.rs   → OpenAI realtime pipeline   │
  │ hotkeys.rs     → global system hotkeys       │
  │ paste.rs       → OS-level auto-paste        │
  │ history.rs     → SQLite storage              │
  │ settings.rs    → JSON settings               │
  │ tray.rs        → system tray integration     │
  │ commands.rs    → Tauri commands              │
  └─────────────────────────────────────────────┘
                       ▲
                       │ IPC calls
                       ▼
  ┌─────────────────────────────────────────────┐
  │             Frontend (React)                │
  ├─────────────────────────────────────────────┤
  │ App.tsx           → Root shell               │
  │ Settings.tsx      → API key, hotkey, config  │
  │ History.tsx       → Transcript list UI       │
  │ RecorderIndicator → Live status overlay      │
  └─────────────────────────────────────────────┘

Key Pipelines
1. Audio → Realtime API

CPAL captures PCM16 frames

Frames are chunked and queued

WebSocket client sends frames as input_audio_buffer.append

When recording ends → input_audio_buffer.commit

Server streams response.output_text.delta events

Final text assembled in memory

2. Output Logic

If active window has a text field → paste text

Otherwise:

history.add(text)

clipboard.set(text)

3. Database Schema
transcripts (
  id INTEGER PRIMARY KEY,
  timestamp INTEGER,
  text TEXT,
  duration_ms INTEGER
)

4. Settings

Stored in JSON:

{
  "api_key": "...",
  "hotkey": "Alt+Space",
  "auto_paste": true,
  "save_history": true,
  "model": "gpt-realtime-mini"
}
