# hanni-chat

Chat, voice, and LLM integration module for Hanni.

## Contains
- **Rust**: chat.rs, prompts.rs, proactive/ (autonomous actions), voice/ (TTS/STT/wake word)
- **JS**: chat.js, chat-input.js, chat-render.js, chat-stream.js, chat-settings.js, chat-overlay.js, conversations.js, actions.js, voice.js
- **CSS**: chat.css, call.css

## Integration
This module is consumed by `hanni-core` at build time. JS files are symlinked/copied into `src/js/`, Rust files into `src-tauri/src/`.
