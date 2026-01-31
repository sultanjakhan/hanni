"""
Clawd Agent Memory System
- Long-term memory (facts, preferences)
- Conversation history
- Semantic search (optional ChromaDB)
"""
import json
import os
from datetime import datetime
from pathlib import Path
from typing import Optional, List, Dict

MEMORY_DIR = Path(__file__).parent / "memory_data"
MEMORY_DIR.mkdir(exist_ok=True)


class Memory:
    """Simple but effective memory system"""

    def __init__(self, user_id: str = "default"):
        self.user_id = user_id
        self.facts_file = MEMORY_DIR / f"{user_id}_facts.json"
        self.history_file = MEMORY_DIR / f"{user_id}_history.json"
        self.load()

    def load(self):
        """Load memory from files"""
        # Facts: key information about user/world
        if self.facts_file.exists():
            self.facts = json.loads(self.facts_file.read_text())
        else:
            self.facts = {
                "user": {},      # Info about user
                "preferences": {},  # User preferences
                "world": {},     # General knowledge
                "tasks": [],     # Ongoing tasks
            }

        # Conversation history
        if self.history_file.exists():
            self.history = json.loads(self.history_file.read_text())
        else:
            self.history = []

    def save(self):
        """Save memory to files"""
        self.facts_file.write_text(json.dumps(self.facts, ensure_ascii=False, indent=2))
        self.history_file.write_text(json.dumps(self.history, ensure_ascii=False, indent=2))

    # === Facts Management ===

    def remember(self, category: str, key: str, value: str):
        """Store a fact"""
        if category not in self.facts:
            self.facts[category] = {}
        self.facts[category][key] = {
            "value": value,
            "timestamp": datetime.now().isoformat()
        }
        self.save()

    def recall(self, category: str, key: str = None) -> Optional[str]:
        """Recall a fact or all facts in category"""
        if category not in self.facts:
            return None
        if key:
            fact = self.facts[category].get(key)
            return fact["value"] if fact else None
        return self.facts[category]

    def forget(self, category: str, key: str):
        """Remove a fact"""
        if category in self.facts and key in self.facts[category]:
            del self.facts[category][key]
            self.save()

    # === Conversation History ===

    def add_message(self, role: str, content: str, metadata: dict = None):
        """Add message to history"""
        self.history.append({
            "role": role,
            "content": content,
            "timestamp": datetime.now().isoformat(),
            "metadata": metadata or {}
        })
        # Keep last 100 messages
        if len(self.history) > 100:
            self.history = self.history[-100:]
        self.save()

    def get_recent_history(self, n: int = 10) -> List[Dict]:
        """Get recent conversation history"""
        return self.history[-n:]

    def search_history(self, query: str) -> List[Dict]:
        """Simple text search in history"""
        query = query.lower()
        return [
            msg for msg in self.history
            if query in msg["content"].lower()
        ]

    # === Context Building ===

    def get_context(self) -> str:
        """Build context string for LLM"""
        context_parts = []

        # User info
        if self.facts.get("user"):
            user_info = []
            for key, data in self.facts["user"].items():
                user_info.append(f"- {key}: {data['value']}")
            if user_info:
                context_parts.append("About the user:\n" + "\n".join(user_info))

        # Preferences
        if self.facts.get("preferences"):
            prefs = []
            for key, data in self.facts["preferences"].items():
                prefs.append(f"- {key}: {data['value']}")
            if prefs:
                context_parts.append("User preferences:\n" + "\n".join(prefs))

        # Active tasks
        if self.facts.get("tasks"):
            tasks = [f"- {t}" for t in self.facts["tasks"]]
            if tasks:
                context_parts.append("Active tasks:\n" + "\n".join(tasks))

        return "\n\n".join(context_parts) if context_parts else ""

    def get_messages_for_llm(self, n: int = 6) -> List[Dict]:
        """Get messages formatted for LLM API"""
        recent = self.get_recent_history(n)
        return [{"role": msg["role"], "content": msg["content"]} for msg in recent]


# Quick test
if __name__ == "__main__":
    mem = Memory("test_user")

    # Store facts
    mem.remember("user", "name", "Sultan")
    mem.remember("user", "language", "Russian and English")
    mem.remember("preferences", "voice", "fast")

    # Add conversation
    mem.add_message("user", "Hello!")
    mem.add_message("assistant", "Hi Sultan! How can I help?")

    # Get context
    print("=== Context ===")
    print(mem.get_context())
    print("\n=== Recent History ===")
    for msg in mem.get_recent_history(5):
        print(f"{msg['role']}: {msg['content']}")
