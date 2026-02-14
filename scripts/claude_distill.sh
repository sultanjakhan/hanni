#!/bin/bash
# Hanni Claude Distillation Pipeline
# Reads conversation history, sends prompts to Claude CLI, generates high-quality training data
# Usage: ./scripts/claude_distill.sh [--focus agentic|personality|all] [--count 20]
set -euo pipefail

HANNI_DIR="$HOME/Library/Application Support/Hanni"
DB_PATH="$HANNI_DIR/hanni.db"
TRAINING_DIR="$HANNI_DIR/training"
DISTILL_DIR="$HANNI_DIR/distillation"
LOG="$HANNI_DIR/distillation.log"
SYSTEM_PROMPT_FILE="$HANNI_DIR/system_prompt.txt"

# Defaults
FOCUS="agentic"
COUNT=15

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --focus) FOCUS="$2"; shift 2 ;;
        --count) COUNT="$2"; shift 2 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"; }

log "=== Claude Distillation started (focus=$FOCUS, count=$COUNT) ==="

# Check claude CLI is available
if ! command -v claude &>/dev/null; then
    log "ERROR: claude CLI not found. Install Claude Code first."
    exit 1
fi

mkdir -p "$DISTILL_DIR"

# Read system prompt
if [ ! -f "$SYSTEM_PROMPT_FILE" ]; then
    log "ERROR: system_prompt.txt not found"
    exit 1
fi
SYSTEM_PROMPT=$(cat "$SYSTEM_PROMPT_FILE")

# Extract user prompts from conversation history
log "Extracting prompts from conversations..."

python3 << PYEOF
import sqlite3, json, os, random

db_path = os.path.expanduser("~/Library/Application Support/Hanni/hanni.db")
distill_dir = os.path.expanduser("~/Library/Application Support/Hanni/distillation")
focus = "$FOCUS"
count = int("$COUNT")

conn = sqlite3.connect(db_path)

# Get all conversations
rows = conn.execute(
    "SELECT id, messages FROM conversations WHERE message_count >= 2 ORDER BY started_at DESC"
).fetchall()

prompts = []
for conv_id, messages_json in rows:
    try:
        messages = json.loads(messages_json)
    except:
        continue
    for i, msg in enumerate(messages):
        role = msg[0] if isinstance(msg, list) else msg.get("role", "")
        content = msg[1] if isinstance(msg, list) else msg.get("content", "")
        if role == "user" and content and not content.startswith("[Action result"):
            # Skip very short or system messages
            clean = content.strip()
            if len(clean) > 5 and not clean.startswith("[Current context]"):
                # Remove [Current context] prefix if present
                if "[Current context]" in clean:
                    parts = clean.split("\n\n", 1)
                    if len(parts) > 1:
                        clean = parts[-1]
                prompts.append(clean)

# Deduplicate
seen = set()
unique = []
for p in prompts:
    key = p[:50].lower()
    if key not in seen:
        seen.add(key)
        unique.append(p)

# Filter by focus
if focus == "agentic":
    # Prefer prompts that should trigger actions
    action_words = [
        "запомни", "запиши", "добавь", "создай", "потратил", "купил", "удали",
        "заблокируй", "трекай", "таймер", "стоп", "настроение", "поел", "спал",
        "открой", "покажи", "сколько", "встреча", "событие", "напомни", "цель",
        "тренировка", "вес", "рецепт", "продукт", "аниме", "фильм", "книг",
        "расход", "доход", "бюджет", "заметк", "синхрон", "календар"
    ]
    scored = []
    for p in unique:
        lower = p.lower()
        score = sum(1 for w in action_words if w in lower)
        scored.append((score, p))
    scored.sort(reverse=True)
    unique = [p for _, p in scored]

# Take top N
selected = unique[:count]

# Also generate synthetic agentic prompts if we don't have enough
synthetic_prompts = [
    "Запомни, что мой любимый язык программирования — Rust",
    "Создай событие на завтра в 14:00 — встреча с ментором",
    "Потратил 8000 на продукты",
    "Начни трекать время — работаю над курсовой",
    "Настроение 3, устал после учёбы",
    "Добавь аниме Attack on Titan, досмотрел, ставлю 9",
    "Заблокируй YouTube и TikTok на 2 часа",
    "Спал 5 часов, выпил 6 стаканов воды",
    "Записала заметку: идеи для стартапа",
    "Что у меня в календаре на эту неделю?",
    "Потренировался 45 минут, бег + растяжка",
    "Купил молоко, срок до пятницы, в холодильник",
    "Сколько я потратил за эту неделю?",
    "Открой GitHub",
    "Запланируй: завтра учёба в 10, обед в 13, спортзал в 18",
    "Удали событие с созвоном",
    "Добавь цель — пробежать 100 км за месяц",
    "Мой друг Димка переехал в Берлин",
    "Завтрак — яичница, примерно 400 калорий",
    "Что сейчас играет в музыке?",
]

if len(selected) < count:
    remaining = count - len(selected)
    selected.extend(synthetic_prompts[:remaining])

# Save prompts for the shell script to process
prompts_file = os.path.join(distill_dir, "prompts.json")
with open(prompts_file, "w") as f:
    json.dump(selected, f, ensure_ascii=False, indent=2)

print(f"Selected {len(selected)} prompts ({len(prompts)} total found, {len(unique)} unique)")
conn.close()
PYEOF

PROMPTS_FILE="$DISTILL_DIR/prompts.json"
if [ ! -f "$PROMPTS_FILE" ]; then
    log "ERROR: No prompts extracted"
    exit 1
fi

PROMPT_COUNT=$(python3 -c "import json; print(len(json.load(open('$PROMPTS_FILE'))))")
log "Processing $PROMPT_COUNT prompts through Claude..."

# Process each prompt through Claude CLI
python3 << 'PYEOF2'
import json, os, subprocess, sys, time

distill_dir = os.path.expanduser("~/Library/Application Support/Hanni/distillation")
training_dir = os.path.expanduser("~/Library/Application Support/Hanni/training")
system_prompt_file = os.path.expanduser("~/Library/Application Support/Hanni/system_prompt.txt")

with open(os.path.join(distill_dir, "prompts.json")) as f:
    prompts = json.load(f)

with open(system_prompt_file) as f:
    system_prompt = f.read().strip()

context = """[Current context]
Today: 2026-02-13, Thursday
Time: 14:30
Active App: Safari"""

results = []
errors = 0

for i, prompt in enumerate(prompts):
    print(f"[{i+1}/{len(prompts)}] Processing: {prompt[:60]}...")

    # Build the distillation prompt for Claude
    claude_prompt = f"""You are helping train a local AI model called Hanni.
Generate the IDEAL response that Hanni should give to this user message.

Hanni's system prompt:
{system_prompt}

Context provided to Hanni:
{context}

User message: {prompt}

Rules for your response:
- Respond AS Hanni (in character: warm, playful, Russian language)
- If the message requires an action, ALWAYS include a ```action block with correct JSON
- Keep responses concise (1-3 sentences + action block)
- Use "ты" form, never "вы"
- Include personality: light humor, genuine warmth
- Output ONLY Hanni's response, nothing else"""

    try:
        result = subprocess.run(
            ["claude", "-p", claude_prompt, "--output-format", "text"],
            capture_output=True, text=True, timeout=60
        )
        if result.returncode == 0 and result.stdout.strip():
            response = result.stdout.strip()
            # Create training example
            example = {
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": f"{context}\n\n{prompt}"},
                    {"role": "assistant", "content": response}
                ]
            }
            results.append(example)
            print(f"  -> OK ({len(response)} chars)")
        else:
            print(f"  -> SKIP (empty or error: {result.stderr[:100]})")
            errors += 1
    except subprocess.TimeoutExpired:
        print(f"  -> TIMEOUT")
        errors += 1
    except Exception as e:
        print(f"  -> ERROR: {e}")
        errors += 1

    # Small delay to respect rate limits
    time.sleep(1)

if not results:
    print("No results generated!")
    sys.exit(1)

# Save distilled examples
distill_file = os.path.join(distill_dir, "distilled.jsonl")
with open(distill_file, "w") as f:
    for ex in results:
        f.write(json.dumps(ex, ensure_ascii=False) + "\n")

print(f"\nDistillation complete: {len(results)} examples, {errors} errors")
print(f"Saved to: {distill_file}")

# Merge with existing training data
existing_train = os.path.join(training_dir, "train.jsonl")
if os.path.exists(existing_train):
    with open(existing_train) as f:
        existing = [json.loads(line) for line in f if line.strip()]
    print(f"Existing training data: {len(existing)} examples")
    merged = existing + results
else:
    merged = results

# Rewrite training splits
total = len(merged)
train_end = max(int(total * 0.8), 1)
valid_end = train_end + max(int(total * 0.1), 1)

for name, data in [("train.jsonl", merged[:train_end]),
                    ("valid.jsonl", merged[train_end:valid_end]),
                    ("test.jsonl", merged[valid_end:])]:
    path = os.path.join(training_dir, name)
    with open(path, "w") as f:
        for ex in data:
            f.write(json.dumps(ex, ensure_ascii=False) + "\n")

print(f"Merged training data: {total} total ({train_end} train, {valid_end-train_end} valid, {total-valid_end} test)")
PYEOF2

log "=== Claude Distillation complete ==="
