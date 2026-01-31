"""
Clawd Agent Core
- Memory integration
- Tool/MCP support
- Multiple interfaces
"""
import asyncio
import json
import httpx
from typing import Optional, List, Dict, Callable
from memory import Memory

# Config
LLM_URL = "http://localhost:8000/v1/chat/completions"

SYSTEM_PROMPT = """You are Clawd, an intelligent AI assistant with memory and tools.

You remember information about the user and past conversations.
You can use tools to search the web, read files, and interact with applications.

When you learn new information about the user, remember it.
Be helpful, concise, and proactive.

{memory_context}
"""


class ClawdAgent:
    """Main agent class with memory and tools"""

    def __init__(self, user_id: str = "default"):
        self.user_id = user_id
        self.memory = Memory(user_id)
        self.tools: Dict[str, Callable] = {}
        self.tool_descriptions: List[Dict] = []

    def register_tool(self, name: str, description: str, func: Callable, parameters: dict = None):
        """Register a tool the agent can use"""
        self.tools[name] = func
        self.tool_descriptions.append({
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": parameters or {"type": "object", "properties": {}}
            }
        })

    async def call_llm(self, messages: List[Dict], use_tools: bool = True) -> Dict:
        """Call the LLM with messages and optional tools"""
        # Build system prompt with memory context
        memory_context = self.memory.get_context()
        system_msg = SYSTEM_PROMPT.format(memory_context=memory_context)

        full_messages = [{"role": "system", "content": system_msg}] + messages

        payload = {
            "messages": full_messages,
            "temperature": 0.7,
            "max_tokens": 1000,
        }

        # Add tools if available and requested
        if use_tools and self.tool_descriptions:
            payload["tools"] = self.tool_descriptions

        async with httpx.AsyncClient(timeout=120.0) as client:
            response = await client.post(LLM_URL, json=payload)

            if response.status_code == 200:
                data = response.json()
                return data["choices"][0]["message"]
            else:
                return {"role": "assistant", "content": f"Error: {response.status_code}"}

    async def execute_tool(self, name: str, arguments: dict) -> str:
        """Execute a registered tool"""
        if name not in self.tools:
            return f"Tool '{name}' not found"

        try:
            func = self.tools[name]
            if asyncio.iscoroutinefunction(func):
                result = await func(**arguments)
            else:
                result = func(**arguments)
            return str(result)
        except Exception as e:
            return f"Tool error: {str(e)}"

    async def chat(self, user_input: str) -> str:
        """Main chat function with memory and tools"""
        # Add user message to memory
        self.memory.add_message("user", user_input)

        # Get recent history for context
        history = self.memory.get_messages_for_llm(6)

        # Call LLM
        response = await self.call_llm(history)

        # Handle tool calls if present
        if response.get("tool_calls"):
            tool_results = []
            for tool_call in response["tool_calls"]:
                func_name = tool_call["function"]["name"]
                try:
                    args = json.loads(tool_call["function"]["arguments"])
                except:
                    args = {}

                result = await self.execute_tool(func_name, args)
                tool_results.append(f"[{func_name}]: {result}")

            # Call LLM again with tool results
            history.append({"role": "assistant", "content": response.get("content", "")})
            history.append({"role": "user", "content": "Tool results:\n" + "\n".join(tool_results)})
            response = await self.call_llm(history, use_tools=False)

        # Extract content
        content = response.get("content", "")
        if not content and response.get("reasoning"):
            # GLM sometimes puts response in reasoning
            content = response["reasoning"].split("\n")[-1].strip()

        if not content:
            content = "I couldn't generate a response."

        # Add assistant response to memory
        self.memory.add_message("assistant", content)

        # Auto-extract and remember facts (simple heuristic)
        self._extract_facts(user_input, content)

        return content

    def _extract_facts(self, user_input: str, response: str):
        """Simple fact extraction from conversation"""
        user_lower = user_input.lower()

        # Detect name
        if "my name is" in user_lower or "i'm " in user_lower or "i am " in user_lower:
            # Simple extraction
            for pattern in ["my name is ", "i'm ", "i am "]:
                if pattern in user_lower:
                    idx = user_lower.index(pattern) + len(pattern)
                    name = user_input[idx:].split()[0].strip(".,!?")
                    if name:
                        self.memory.remember("user", "name", name)
                        break

        # Detect preferences
        if "i prefer" in user_lower or "i like" in user_lower:
            # Store as preference hint
            self.memory.remember("preferences", "noted", user_input[:100])


# === Built-in Tools ===

def search_memory(query: str) -> str:
    """Search in agent's memory"""
    # This would be implemented with the Memory class
    return f"Searching memory for: {query}"


def remember_fact(category: str, key: str, value: str) -> str:
    """Store a fact in memory"""
    return f"Remembered: {category}/{key} = {value}"


# Quick test
if __name__ == "__main__":
    async def test():
        agent = ClawdAgent("test")

        # Register a simple tool
        agent.register_tool(
            "get_time",
            "Get current time",
            lambda: __import__('datetime').datetime.now().strftime("%H:%M:%S")
        )

        print("Clawd Agent ready. Type 'quit' to exit.\n")

        while True:
            user_input = input("You: ").strip()
            if user_input.lower() in ["quit", "exit", "q"]:
                break
            if not user_input:
                continue

            response = await agent.chat(user_input)
            print(f"Clawd: {response}\n")

    asyncio.run(test())
