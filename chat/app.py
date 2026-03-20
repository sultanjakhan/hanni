#!/usr/bin/env python3
"""Hanni Chat — fast streaming chat app talking directly to mlx-lm.server."""

import json
import aiohttp
from aiohttp import web

MLX_URL = "http://127.0.0.1:8234/v1/chat/completions"
MODEL = "mlx-community/Qwen3-30B-A3B-4bit"
SYSTEM_PROMPT = (
    "You are Hanni, a helpful AI assistant. "
    "Answer concisely and directly. Use the same language as the user. /no_think"
)

# In-memory conversation history per session (simple dict by session id)
sessions: dict[str, list[dict]] = {}


async def index(request: web.Request) -> web.Response:
    return web.FileResponse("index.html")


async def chat(request: web.Request) -> web.StreamResponse:
    raw = await request.read()
    data = json.loads(raw.decode("utf-8", errors="replace"))
    user_msg = data.get("message", "")
    session_id = data.get("session_id", "default")

    # Get or create conversation history
    history = sessions.setdefault(session_id, [])
    history.append({"role": "user", "content": user_msg})

    # Keep last 20 messages to avoid context overflow
    messages = [{"role": "system", "content": SYSTEM_PROMPT}] + history[-20:]

    payload = {
        "model": MODEL,
        "messages": messages,
        "max_tokens": 1024,
        "stream": True,
    }

    response = web.StreamResponse()
    response.content_type = "text/event-stream"
    response.headers["Cache-Control"] = "no-cache"
    response.headers["X-Accel-Buffering"] = "no"
    await response.prepare(request)

    full_reply = ""
    in_think = False

    async with aiohttp.ClientSession() as session:
        async with session.post(
            MLX_URL,
            json=payload,
            headers={"Content-Type": "application/json"},
            timeout=aiohttp.ClientTimeout(total=120),
        ) as resp:
            async for line in resp.content:
                text = line.decode("utf-8").strip()
                if not text or not text.startswith("data: "):
                    continue
                chunk = text[6:]
                if chunk == "[DONE]":
                    await response.write(b"data: [DONE]\n\n")
                    break
                try:
                    obj = json.loads(chunk)
                    delta = obj["choices"][0].get("delta", {})
                    token = delta.get("content", "")
                    if not token:
                        continue
                    # Filter out <think>...</think> blocks
                    if "<think>" in token:
                        in_think = True
                        continue
                    if "</think>" in token:
                        in_think = False
                        continue
                    if in_think:
                        continue
                    full_reply += token
                    await response.write(
                        f"data: {json.dumps({'token': token})}\n\n".encode()
                    )
                except (json.JSONDecodeError, KeyError, IndexError):
                    continue

    # Save assistant reply to history
    if full_reply:
        history.append({"role": "assistant", "content": full_reply})

    return response


app = web.Application()
app.router.add_get("/", index)
app.router.add_post("/api/chat", chat)

if __name__ == "__main__":
    print("Hanni Chat running at http://localhost:8080")
    web.run_app(app, host="127.0.0.1", port=8080, print=None)
