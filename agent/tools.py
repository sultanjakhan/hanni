"""
Clawd Agent Tools
- Web search
- File operations
- System info
- Custom app integrations
"""
import os
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Optional
import httpx


# === Basic Tools ===

def get_current_time() -> str:
    """Get current date and time"""
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def get_system_info() -> str:
    """Get basic system information"""
    import platform
    return f"OS: {platform.system()} {platform.release()}, Python: {platform.python_version()}"


# === File Tools ===

def read_file(path: str) -> str:
    """Read a file's contents"""
    try:
        p = Path(path).expanduser()
        if not p.exists():
            return f"File not found: {path}"
        if p.stat().st_size > 100000:  # 100KB limit
            return f"File too large: {p.stat().st_size} bytes"
        return p.read_text()[:10000]  # Max 10K chars
    except Exception as e:
        return f"Error reading file: {e}"


def write_file(path: str, content: str) -> str:
    """Write content to a file"""
    try:
        p = Path(path).expanduser()
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content)
        return f"Written {len(content)} bytes to {path}"
    except Exception as e:
        return f"Error writing file: {e}"


def list_directory(path: str = ".") -> str:
    """List files in a directory"""
    try:
        p = Path(path).expanduser()
        if not p.exists():
            return f"Directory not found: {path}"
        files = list(p.iterdir())[:50]  # Max 50 items
        return "\n".join([f"{'[DIR]' if f.is_dir() else '[FILE]'} {f.name}" for f in files])
    except Exception as e:
        return f"Error listing directory: {e}"


# === Web Tools ===

async def web_search(query: str) -> str:
    """Search the web using DuckDuckGo"""
    try:
        # Simple DuckDuckGo instant answer API
        async with httpx.AsyncClient(timeout=10.0) as client:
            response = await client.get(
                "https://api.duckduckgo.com/",
                params={"q": query, "format": "json", "no_html": 1}
            )
            data = response.json()

            results = []
            if data.get("AbstractText"):
                results.append(f"Summary: {data['AbstractText']}")
            if data.get("RelatedTopics"):
                for topic in data["RelatedTopics"][:3]:
                    if isinstance(topic, dict) and topic.get("Text"):
                        results.append(f"- {topic['Text'][:200]}")

            return "\n".join(results) if results else "No results found"
    except Exception as e:
        return f"Search error: {e}"


async def fetch_url(url: str) -> str:
    """Fetch content from a URL"""
    try:
        async with httpx.AsyncClient(timeout=15.0, follow_redirects=True) as client:
            response = await client.get(url)
            # Return first 5000 chars of text content
            content = response.text[:5000]
            return content
    except Exception as e:
        return f"Fetch error: {e}"


# === Shell Tools (limited) ===

def run_command(command: str) -> str:
    """Run a safe shell command"""
    # Whitelist of safe commands
    safe_prefixes = ["ls", "pwd", "date", "whoami", "echo", "cat", "head", "tail"]

    cmd_start = command.split()[0] if command.split() else ""
    if not any(command.startswith(p) for p in safe_prefixes):
        return f"Command not allowed: {cmd_start}"

    try:
        result = subprocess.run(
            command,
            shell=True,
            capture_output=True,
            text=True,
            timeout=10
        )
        output = result.stdout or result.stderr
        return output[:2000] if output else "No output"
    except subprocess.TimeoutExpired:
        return "Command timed out"
    except Exception as e:
        return f"Command error: {e}"


# === Tool Registry ===

TOOLS = {
    "get_time": {
        "func": get_current_time,
        "description": "Get current date and time",
        "parameters": {}
    },
    "get_system_info": {
        "func": get_system_info,
        "description": "Get system information",
        "parameters": {}
    },
    "read_file": {
        "func": read_file,
        "description": "Read contents of a file",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path to read"}
            },
            "required": ["path"]
        }
    },
    "write_file": {
        "func": write_file,
        "description": "Write content to a file",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path to write"},
                "content": {"type": "string", "description": "Content to write"}
            },
            "required": ["path", "content"]
        }
    },
    "list_directory": {
        "func": list_directory,
        "description": "List files in a directory",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Directory path"}
            }
        }
    },
    "web_search": {
        "func": web_search,
        "description": "Search the web for information",
        "parameters": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Search query"}
            },
            "required": ["query"]
        }
    },
    "fetch_url": {
        "func": fetch_url,
        "description": "Fetch content from a URL",
        "parameters": {
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "URL to fetch"}
            },
            "required": ["url"]
        }
    },
    "run_command": {
        "func": run_command,
        "description": "Run a safe shell command (ls, pwd, date, echo, cat, head, tail)",
        "parameters": {
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command to run"}
            },
            "required": ["command"]
        }
    }
}


def register_all_tools(agent):
    """Register all tools with an agent"""
    for name, tool in TOOLS.items():
        agent.register_tool(
            name=name,
            description=tool["description"],
            func=tool["func"],
            parameters=tool.get("parameters")
        )
