# Chat

## Purpose
AI assistant interface with persistent memory, conversation history, and model fine-tuning pipeline. The central interaction point of Hanni.

## DB Tables

| Table | Purpose |
|-------|---------|
| `conversations` | Conversation threads (title, messages JSON, timestamps) |
| `conversations_fts` | Full-text search index for conversations |
| `facts` | Extracted facts from conversations (semantic memory) |
| `facts_fts` | Full-text search index for facts |
| `vec_facts` | Vector embeddings for semantic fact search |
| `memory_decay` | Access tracking for memory relevance scoring |
| `flywheel_cycles` | Training data collection cycles |
| `message_feedback` | User feedback on AI responses (thumbs up/down) |
| `conversation_insights` | AI-generated conversation summaries |

## Views
- Single chat view with conversation list sidebar
- Memory management panel (view/edit/delete facts)
- Training stats dashboard

## Key Commands (Tauri)
- `chat` — send message, get streaming SSE response
- `get_conversations` / `save_conversation` / `update_conversation` — CRUD
- `get_all_memories` / `memory_remember` / `update_memory` / `delete_memory` — fact management
- `get_training_stats` / `get_flywheel_status` — training pipeline status

## Relations
- Facts extracted from chat are used across all tabs for context
- Calendar events and schedule items can be created from chat actions
- Proactive messages feed into chat

## Notable
- Streaming via SSE: `chat-token`, `chat-done`, `chat-reasoning`, `chat-reasoning-done`
- Local LLM (Qwen3.5-35B-A3B via MLX at `127.0.0.1:8234`)
- Voice input/output support via `voice_server.py`
- Flywheel: collects training data from conversations for model fine-tuning
