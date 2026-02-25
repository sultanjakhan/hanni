#!/usr/bin/env python3
"""
Hanni Training Data Pipeline.

1. Exports good conversations from DB (positive feedback, no hallucinations)
2. Merges with curated synthetic examples
3. Validates format
4. Splits into train/valid/test (80/10/10)
5. Writes to ~/Library/Application Support/Hanni/training/

Usage:
    python3 prepare_data.py [--export-only] [--synthetic-only] [--dry-run]
"""

import argparse
import json
import os
import random
import re
import sqlite3
import sys
from pathlib import Path

DB_PATH = Path.home() / "Library" / "Application Support" / "Hanni" / "hanni.db"
OUTPUT_DIR = Path.home() / "Library" / "Application Support" / "Hanni" / "training"
SYNTHETIC_DIR = Path(__file__).parent

# Current Russian system prompt (must match lib.rs SYSTEM_PROMPT)
SYSTEM_PROMPT = """Ты — Ханни, тёплый и любопытный AI-компаньон на Mac. Близкий друг, который искренне заботится. Отвечай кратко, но выразительно. На "ты", по-русски.

ИНСТРУМЕНТЫ:
- Когда пользователь просит что-то СДЕЛАТЬ — ВСЕГДА вызывай инструмент.
- "запомни", "запиши", "добавь", "потратил" → инструмент. НИКОГДА не говори "ок" без действия!
- Можно вызывать несколько инструментов за раз.
- Даты: считай от [Current context] Today. "завтра"=Today+1, "послезавтра"=Today+2. Формат YYYY-MM-DD.
- Целодневные события: create_event с time="" и duration=0.
- Запоминай важные факты (имя, предпочтения, привычки, люди) через remember.
- Память уже в контексте — search_memory только для конкретных запросов.
- После результатов инструмента — резюмируй естественно. НЕ повторяй сырой вывод.
- web_search для актуальной информации: факты, рецепты, цены, погода, новости.

СТИЛЬ:
- Тёплый тон: лёгкий юмор, любопытство, игривый сарказм (по-доброму).
- Разнообразь формат: иногда вопрос, иногда шутка, иногда наблюдение. НЕ начинай каждый ответ одинаково.
- Из памяти вплетай естественно: "Ты же вроде учишься в KBTU..." а не "Согласно моей памяти..."

КАЧЕСТВО:
- Сложный вопрос → продумай пошагово, потом отвечай.
- Эмоция → сначала отреагируй на чувство, потом совет.
- Неясный запрос → задай ОДИН уточняющий вопрос.
- Простой вопрос = 1-2 предложения. Сложный = 3-6, со структурой.

СТРОГИЕ ЗАПРЕТЫ (нарушение = критическая ошибка):
- ЗАПРЕЩЕНО выдумывать факты, события, привычки, предпочтения которых нет в памяти. Если не знаешь — скажи: "Не помню", "Не знаю", "Расскажи".
- ЗАПРЕЩЕНО упоминать еду, напитки, чай, кофе, чайник, перекусы — если пользователь НЕ спрашивает о еде.
- ЗАПРЕЩЕНО придумывать что пользователь делал, говорил или любит — если этого нет в [Релевантные факты].
- Используй факты из памяти ТОЛЬКО если релевантны текущему вопросу. НЕ перечисляй всё подряд.
- На "привет" — ответь коротко и тепло. Без придуманных подробностей.
- НЕ повторяй сообщение пользователя."""

# Hallucination patterns to filter out
HALLUCINATION_PATTERNS = [
    r"чайник", r"чаёк", r"чайку", r"заварить чай",
    r"кофейку", r"кофеёк", r"латте", r"капучино",
    r"в гущу разработки", r"опять в электрон",
    r"OpenClaw", r"openclaw",
]


def has_hallucination(text: str) -> bool:
    lower = text.lower()
    for pat in HALLUCINATION_PATTERNS:
        if re.search(pat, lower):
            return True
    return False


def export_from_db(min_messages=4) -> list[dict]:
    """Export good conversations from the database."""
    if not DB_PATH.exists():
        print(f"WARNING: DB not found at {DB_PATH}")
        return []

    conn = sqlite3.connect(str(DB_PATH))
    conn.row_factory = sqlite3.Row

    # Load feedback
    feedback_map: dict[int, dict[int, int]] = {}
    for row in conn.execute("SELECT conversation_id, message_index, rating FROM message_feedback"):
        cid = row["conversation_id"]
        if cid not in feedback_map:
            feedback_map[cid] = {}
        feedback_map[cid][row["message_index"]] = row["rating"]

    # Load conversations
    rows = conn.execute(
        "SELECT id, messages, message_count FROM conversations "
        "WHERE message_count >= ? ORDER BY started_at",
        (min_messages,),
    ).fetchall()

    examples = []
    skipped_halluc = 0
    skipped_neg = 0

    for row in rows:
        cid = row["id"]
        try:
            messages = json.loads(row["messages"])
        except json.JSONDecodeError:
            continue

        ratings = feedback_map.get(cid, {})
        neg_count = sum(1 for v in ratings.values() if v == -1)
        pos_count = sum(1 for v in ratings.values() if v == 1)

        # Skip conversations with mostly negative feedback
        if neg_count > pos_count + 2:
            skipped_neg += 1
            continue

        # Build training messages
        chat_msgs = [{"role": "system", "content": SYSTEM_PROMPT}]
        has_halluc = False

        for idx, msg in enumerate(messages):
            # Handle both formats: [role, content] array and {role, content} dict
            if isinstance(msg, list):
                role, content = msg[0], msg[1] if len(msg) > 1 else ""
            elif isinstance(msg, dict):
                role = msg.get("role", "")
                content = msg.get("content", "") or ""
            else:
                continue

            if role not in ("user", "assistant"):
                continue

            # Skip negatively-rated assistant messages
            if role == "assistant" and ratings.get(idx) == -1:
                continue

            # Check for hallucinations in assistant messages
            if role == "assistant" and has_hallucination(content):
                has_halluc = True
                break

            # Clean content
            clean = content.strip()
            clean = re.sub(r'\s*/no_think\s*$', '', clean)
            if not clean:
                continue

            chat_msgs.append({"role": role, "content": clean})

        if has_halluc:
            skipped_halluc += 1
            continue

        # Must have at least 2 real messages (user + assistant)
        real = [m for m in chat_msgs if m["role"] in ("user", "assistant")]
        if len(real) < 2:
            continue

        examples.append({"messages": chat_msgs})

    conn.close()
    print(f"DB export: {len(examples)} examples (skipped {skipped_halluc} hallucinations, {skipped_neg} negative)")
    return examples


def load_synthetic() -> list[dict]:
    """Load curated synthetic training examples."""
    synthetic_file = SYNTHETIC_DIR / "synthetic_examples.jsonl"
    if not synthetic_file.exists():
        print(f"WARNING: {synthetic_file} not found")
        return []

    examples = []
    with open(synthetic_file) as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                data = json.loads(line)
                # Replace placeholder or add system prompt
                msgs = data.get("messages", [])
                if msgs and msgs[0].get("role") == "system":
                    # Replace placeholder "..." with real prompt
                    if msgs[0].get("content") == "...":
                        msgs[0]["content"] = SYSTEM_PROMPT
                else:
                    msgs.insert(0, {"role": "system", "content": SYSTEM_PROMPT})
                data["messages"] = msgs
                examples.append(data)
            except json.JSONDecodeError as e:
                print(f"WARNING: Invalid JSON at line {line_num}: {e}")

    print(f"Synthetic: {len(examples)} examples loaded")
    return examples


def validate_examples(examples: list[dict]) -> list[dict]:
    """Validate and clean training examples."""
    valid = []
    for i, ex in enumerate(examples):
        msgs = ex.get("messages", [])

        # Must have system + at least user + assistant
        if len(msgs) < 3:
            continue

        # System prompt must be first
        if msgs[0].get("role") != "system":
            continue

        # Must have at least one user and one assistant message
        roles = [m["role"] for m in msgs[1:]]
        if "user" not in roles or "assistant" not in roles:
            continue

        # Check for hallucinations in all assistant messages
        has_halluc = False
        for m in msgs:
            if m["role"] == "assistant" and has_hallucination(m["content"]):
                has_halluc = True
                break
        if has_halluc:
            continue

        # No empty content
        if any(not m.get("content", "").strip() for m in msgs):
            continue

        valid.append(ex)

    print(f"Validation: {len(valid)}/{len(examples)} passed")
    return valid


def split_and_write(examples: list[dict], dry_run=False):
    """Shuffle and split into train/valid/test, then write."""
    random.seed(42)
    random.shuffle(examples)

    total = len(examples)
    train_end = int(total * 0.8)
    valid_end = train_end + int(total * 0.1)

    # Ensure at least 1 in each split
    if total >= 3:
        train_end = max(train_end, 1)
        valid_end = max(valid_end, train_end + 1)
        valid_end = min(valid_end, total - 1)

    train = examples[:train_end]
    valid = examples[train_end:valid_end]
    test = examples[valid_end:]

    print(f"\nSplit: {len(train)} train, {len(valid)} valid, {len(test)} test")

    if dry_run:
        print("(dry run — not writing files)")
        return

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    for name, data in [("train.jsonl", train), ("valid.jsonl", valid), ("test.jsonl", test)]:
        path = OUTPUT_DIR / name
        with open(path, "w") as f:
            for ex in data:
                f.write(json.dumps(ex, ensure_ascii=False) + "\n")
        print(f"  Wrote {path} ({len(data)} examples)")


def main():
    parser = argparse.ArgumentParser(description="Hanni Training Data Pipeline")
    parser.add_argument("--export-only", action="store_true", help="Only export from DB")
    parser.add_argument("--synthetic-only", action="store_true", help="Only use synthetic data")
    parser.add_argument("--dry-run", action="store_true", help="Don't write files")
    args = parser.parse_args()

    examples = []

    if not args.synthetic_only:
        db_examples = export_from_db()
        examples.extend(db_examples)

    if not args.export_only:
        synthetic = load_synthetic()
        examples.extend(synthetic)

    if not examples:
        print("ERROR: No training examples found!")
        sys.exit(1)

    examples = validate_examples(examples)

    if not examples:
        print("ERROR: No valid examples after filtering!")
        sys.exit(1)

    split_and_write(examples, dry_run=args.dry_run)
    print(f"\nTotal: {len(examples)} training examples ready")


if __name__ == "__main__":
    main()
