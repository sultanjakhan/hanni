---
name: debug-llm
description: Debug the LLM pipeline — check MLX server status, test prompts, inspect system prompt, analyze response quality.
allowed-tools: Bash, Read, Grep, Glob, Task, WebFetch
argument-hint: [area]
user-invocable: true
---

# Debug LLM Pipeline

Diagnose and debug LLM-related issues in Hanni.

## Input

Optional area to focus on: `server`, `prompt`, `actions`, `memory`, `quality`, or empty for full check.

## Steps

### 1. Server Health Check
```bash
curl -s http://127.0.0.1:8234/v1/models | python3 -m json.tool
```
- Is MLX server running?
- What model is loaded?
- What's the response time?

### 2. System Prompt Analysis
- Read the SYSTEM_PROMPT from `main.js`
- Count tokens (approximate: chars / 4)
- Check for:
  - Clarity of instructions
  - Action examples completeness
  - Personality consistency
  - Context window budget (how much room for conversation?)

### 3. Test LLM Call
```bash
curl -s http://127.0.0.1:8234/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"mlx-community/Qwen3-32B-4bit","messages":[{"role":"user","content":"Привет, как дела?"}],"max_tokens":100,"temperature":0.7}'
```
- Does it respond?
- Is the response in Russian?
- Is the quality acceptable?

### 4. Action Generation Test
- Send a prompt that should trigger an action
- Check if the model outputs proper ```action JSON
- Verify the JSON is valid and the action type exists

### 5. Memory Integration Check
- Check how memories are injected into the prompt
- Verify memory retrieval is working
- Check if memories are relevant to queries

### 6. Report findings:
- Server status: OK/DOWN
- Model: loaded/not loaded
- Response quality: good/degraded/bad
- Action generation: reliable/flaky
- Memory: working/broken
- Token budget: X tokens used by system prompt, Y available for conversation

## Rules

- Respond in Russian
- Don't modify any code — this is diagnostic only
- If the server is down, suggest restart commands
- If quality is poor, suggest prompt improvements
- Consider hardware constraints: M3 Pro 36GB, model uses ~17GB
