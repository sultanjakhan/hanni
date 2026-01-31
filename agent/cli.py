#!/usr/bin/env python3
"""
Clawd Agent CLI
Interactive command-line interface
"""
import asyncio
import sys
from pathlib import Path

# Add parent to path for imports
sys.path.insert(0, str(Path(__file__).parent))

from core import ClawdAgent
from tools import register_all_tools
from google_tools import register_google_tools
from homefit_tools import register_homefit_tools
from lifetracker_tools import register_lifetracker_tools


def print_banner():
    print("""
╔═══════════════════════════════════════╗
║         CLAWD AGENT v0.1              ║
║   Memory + Tools + Intelligence       ║
╠═══════════════════════════════════════╣
║ Commands:                             ║
║   /memory  - Show memory contents     ║
║   /forget  - Clear memory             ║
║   /tools   - List available tools     ║
║   /quit    - Exit                     ║
╚═══════════════════════════════════════╝
""")


async def main():
    print_banner()

    # Initialize agent
    agent = ClawdAgent(user_id="main")
    register_all_tools(agent)
    register_google_tools(agent)
    register_homefit_tools(agent)
    register_lifetracker_tools(agent)

    print(f"Loaded {len(agent.tools)} tools: {', '.join(agent.tools.keys())}")
    print(f"Memory context:\n{agent.memory.get_context() or '(empty)'}\n")
    print("Ready! Type your message or /quit to exit.\n")

    while True:
        try:
            user_input = input("You: ").strip()
        except (KeyboardInterrupt, EOFError):
            print("\nGoodbye!")
            break

        if not user_input:
            continue

        # Handle commands
        if user_input.startswith("/"):
            cmd = user_input.lower()

            if cmd in ["/quit", "/exit", "/q"]:
                print("Goodbye!")
                break

            elif cmd == "/memory":
                print("\n=== Memory Contents ===")
                print(f"Facts: {agent.memory.facts}")
                print(f"History: {len(agent.memory.history)} messages")
                print(f"Context:\n{agent.memory.get_context() or '(empty)'}")
                print()
                continue

            elif cmd == "/forget":
                agent.memory.facts = {"user": {}, "preferences": {}, "world": {}, "tasks": []}
                agent.memory.history = []
                agent.memory.save()
                print("Memory cleared.\n")
                continue

            elif cmd == "/tools":
                print("\n=== Available Tools ===")
                for name, tool in agent.tools.items():
                    print(f"  {name}")
                print()
                continue

            elif cmd == "/history":
                print("\n=== Recent History ===")
                for msg in agent.memory.get_recent_history(10):
                    role = msg["role"]
                    content = msg["content"][:100] + "..." if len(msg["content"]) > 100 else msg["content"]
                    print(f"  [{role}]: {content}")
                print()
                continue

            else:
                print(f"Unknown command: {cmd}")
                continue

        # Chat with agent
        print("Clawd: ", end="", flush=True)
        try:
            response = await agent.chat(user_input)
            print(response)
        except Exception as e:
            print(f"Error: {e}")
        print()


if __name__ == "__main__":
    asyncio.run(main())
