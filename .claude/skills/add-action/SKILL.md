---
name: add-action
description: Scaffold a new AI action type — JS handler in main.js, example in system prompt, and Rust command if needed.
allowed-tools: Read, Edit, Grep, Glob, Task
argument-hint: <action_name> [description]
user-invocable: true
---

# Scaffold New AI Action

Create a new action type that the LLM can trigger via ```action blocks.

## Input

- First argument: action name (e.g., `set_reminder`, `play_music`)
- Optional: description of what the action does

## Steps

1. **Understand the action** — what it does, what parameters it needs, what the user sees

2. **Read current patterns** — find existing action handlers in `main.js`:
   - Search for the action parsing/dispatch logic
   - Study 2-3 existing action handlers for the pattern
   - Check the SYSTEM_PROMPT for existing action examples

3. **Add JS action handler** in `desktop/src/main.js`:
   - Add case to the action dispatch switch/if-chain
   - Implement the handler following existing patterns
   - Add any UI feedback (toast, animation, etc.)

4. **Add system prompt example** — add a few-shot example to SYSTEM_PROMPT showing the LLM how to use this action:
   ```
   User: [example request]
   Assistant: [natural response]
   \`\`\`action
   {"type": "action_name", "param1": "value1"}
   \`\`\`
   ```

5. **Add Rust command if needed** — if the action requires backend work:
   - Create the Tauri command (follow `/add-command` patterns)
   - Register it in invoke_handler

6. **Report** the full action flow: user says X → LLM outputs action → JS handles → result

## Rules

- Respond in Russian
- The LLM is Qwen3-32B-4bit — keep action JSON simple, the model handles simple structures best
- Always add a system prompt example — without it the model won't know to use the action
- Match existing action patterns exactly
- Consider error cases: what if the action fails? Show user-friendly feedback
