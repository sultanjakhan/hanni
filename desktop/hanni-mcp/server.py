#!/usr/bin/env python3
"""
Hanni MCP Server — gives Claude Code direct access to Hanni's data.

Tools:
  - get_facts: Read all memory facts (optionally filtered)
  - get_conversations: List recent conversations
  - get_conversation: Read a specific conversation's messages
  - get_proactive_history: Recent proactive messages
  - get_settings: App settings
  - test_chat: Send a test message through MLX and get response
  - run_sql: Run a read-only SQL query on hanni.db
"""
import json, os, sys, sqlite3, time
import requests

# MCP protocol over stdio
DB_PATH = os.path.expanduser("~/Library/Application Support/Hanni/hanni.db")
MLX_URL = "http://127.0.0.1:8234/v1/chat/completions"
MLX_MODEL = "mlx-community/Qwen3-32B-4bit"


def get_db():
    conn = sqlite3.connect(DB_PATH, timeout=5)
    conn.row_factory = sqlite3.Row
    return conn


def handle_get_facts(args):
    conn = get_db()
    search = args.get("search", "")
    if search:
        like = f"%{search}%"
        rows = conn.execute(
            "SELECT id, category, key, value, source, updated_at FROM facts "
            "WHERE key LIKE ? OR value LIKE ? OR category LIKE ? "
            "ORDER BY updated_at DESC LIMIT 100",
            (like, like, like)
        ).fetchall()
    else:
        rows = conn.execute(
            "SELECT id, category, key, value, source, updated_at FROM facts "
            "ORDER BY category, updated_at DESC LIMIT 200"
        ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def handle_get_conversations(args):
    conn = get_db()
    limit = args.get("limit", 20)
    rows = conn.execute(
        "SELECT id, summary, message_count, created_at, ended_at FROM conversations "
        "ORDER BY id DESC LIMIT ?", (limit,)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def handle_get_conversation(args):
    conn = get_db()
    cid = args.get("id")
    if not cid:
        return {"error": "id required"}
    row = conn.execute(
        "SELECT id, summary, messages, message_count, created_at FROM conversations WHERE id=?",
        (cid,)
    ).fetchone()
    conn.close()
    if not row:
        return {"error": "not found"}
    result = dict(row)
    try:
        result["messages"] = json.loads(result["messages"])
    except:
        pass
    return result


def handle_get_proactive_history(args):
    conn = get_db()
    limit = args.get("limit", 30)
    rows = conn.execute(
        "SELECT id, message, sent_at, engagement FROM proactive_history "
        "ORDER BY id DESC LIMIT ?", (limit,)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def handle_get_settings(args):
    conn = get_db()
    rows = conn.execute("SELECT key, value FROM app_settings").fetchall()
    conn.close()
    return {r["key"]: r["value"] for r in rows}


def handle_test_chat(args):
    message = args.get("message", "Привет!")
    system = args.get("system", None)

    if not system:
        # Build system prompt with real facts
        conn = get_db()
        facts = conn.execute(
            "SELECT category, key, value FROM facts ORDER BY category, updated_at DESC LIMIT 20"
        ).fetchall()
        conn.close()
        memory = "\n".join(f"[{r['category']}] {r['key']}={r['value']}" for r in facts)
        system = (
            "Ты — Ханни, тёплый AI-компаньон. Отвечай кратко, на \"ты\", по-русски.\n"
            f"[Memory]\n{memory}"
        )

    start = time.time()
    try:
        resp = requests.post(MLX_URL, json={
            "model": MLX_MODEL,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": message},
            ],
            "max_tokens": 300,
            "temperature": 0.7,
            "stream": False,
            "chat_template_kwargs": {"enable_thinking": False},
        }, timeout=60)
        elapsed = time.time() - start
        data = resp.json()
        import re
        raw = data["choices"][0]["message"]["content"]
        text = re.sub(r'(?s)<think>.*?</think>', '', raw).strip()
        tokens = data.get("usage", {}).get("completion_tokens", "?")
        return {
            "response": text,
            "elapsed_seconds": round(elapsed, 1),
            "tokens": tokens,
            "model": MLX_MODEL,
        }
    except Exception as e:
        return {"error": str(e)}


def handle_run_sql(args):
    query = args.get("query", "")
    if not query:
        return {"error": "query required"}
    # Safety: only allow SELECT
    if not query.strip().upper().startswith("SELECT"):
        return {"error": "only SELECT queries allowed"}
    conn = get_db()
    try:
        rows = conn.execute(query).fetchall()
        return [dict(r) for r in rows]
    except Exception as e:
        return {"error": str(e)}
    finally:
        conn.close()


TOOLS = {
    "get_facts": {
        "description": "Get all memory facts from Hanni's database. Optionally filter by search term.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "search": {"type": "string", "description": "Optional search filter"}
            }
        },
        "handler": handle_get_facts,
    },
    "get_conversations": {
        "description": "List recent conversations with summaries.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "limit": {"type": "number", "description": "Max results (default 20)"}
            }
        },
        "handler": handle_get_conversations,
    },
    "get_conversation": {
        "description": "Get full conversation messages by ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "number", "description": "Conversation ID"}
            },
            "required": ["id"]
        },
        "handler": handle_get_conversation,
    },
    "get_proactive_history": {
        "description": "Get recent proactive messages sent by Hanni.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "limit": {"type": "number", "description": "Max results (default 30)"}
            }
        },
        "handler": handle_get_proactive_history,
    },
    "get_settings": {
        "description": "Get all Hanni app settings.",
        "inputSchema": {"type": "object", "properties": {}},
        "handler": handle_get_settings,
    },
    "test_chat": {
        "description": "Send a test message to the MLX server with Hanni's prompt and real memory. Returns response + timing.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "message": {"type": "string", "description": "User message to send"},
                "system": {"type": "string", "description": "Optional custom system prompt (default: Hanni lite + real facts)"}
            },
            "required": ["message"]
        },
        "handler": handle_test_chat,
    },
    "run_sql": {
        "description": "Run a read-only SELECT query on hanni.db.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "SQL SELECT query"}
            },
            "required": ["query"]
        },
        "handler": handle_run_sql,
    },
}


def send_response(id, result):
    response = {"jsonrpc": "2.0", "id": id, "result": result}
    msg = json.dumps(response)
    sys.stdout.write(f"Content-Length: {len(msg.encode())}\r\n\r\n{msg}")
    sys.stdout.flush()


def send_error(id, code, message):
    response = {"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}
    msg = json.dumps(response)
    sys.stdout.write(f"Content-Length: {len(msg.encode())}\r\n\r\n{msg}")
    sys.stdout.flush()


def main():
    """MCP server over stdio using JSON-RPC."""
    buf = ""
    while True:
        try:
            line = sys.stdin.readline()
            if not line:
                break

            # Parse Content-Length header
            if line.startswith("Content-Length:"):
                length = int(line.split(":")[1].strip())
                sys.stdin.readline()  # empty line
                body = sys.stdin.read(length)
                request = json.loads(body)
            else:
                buf += line
                # Try to parse as JSON directly (some clients skip headers)
                try:
                    request = json.loads(buf)
                    buf = ""
                except json.JSONDecodeError:
                    continue

            method = request.get("method", "")
            id = request.get("id")
            params = request.get("params", {})

            if method == "initialize":
                send_response(id, {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "hanni-mcp", "version": "1.0.0"},
                })
            elif method == "notifications/initialized":
                pass  # no response needed
            elif method == "tools/list":
                tools_list = []
                for name, tool in TOOLS.items():
                    tools_list.append({
                        "name": name,
                        "description": tool["description"],
                        "inputSchema": tool["inputSchema"],
                    })
                send_response(id, {"tools": tools_list})
            elif method == "tools/call":
                tool_name = params.get("name", "")
                tool_args = params.get("arguments", {})
                if tool_name in TOOLS:
                    try:
                        result = TOOLS[tool_name]["handler"](tool_args)
                        send_response(id, {
                            "content": [{"type": "text", "text": json.dumps(result, ensure_ascii=False, indent=2)}]
                        })
                    except Exception as e:
                        send_response(id, {
                            "content": [{"type": "text", "text": f"Error: {e}"}],
                            "isError": True,
                        })
                else:
                    send_error(id, -32601, f"Unknown tool: {tool_name}")
            elif method == "ping":
                send_response(id, {})
            else:
                if id is not None:
                    send_error(id, -32601, f"Method not found: {method}")

        except Exception as e:
            sys.stderr.write(f"MCP error: {e}\n")
            sys.stderr.flush()


if __name__ == "__main__":
    main()
