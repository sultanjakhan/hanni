#!/bin/bash
# Hanni Nightly LoRA Training Script
# Runs at 3 AM via LaunchAgent — exports training data, fine-tunes, restarts MLX server
set -euo pipefail

HANNI_DIR="$HOME/Library/Application Support/Hanni"
DB_PATH="$HANNI_DIR/hanni.db"
TRAINING_DIR="$HANNI_DIR/training"
ADAPTER_DIR="$HANNI_DIR/lora-adapter"
ADAPTER_BACKUP="$HANNI_DIR/lora-adapter-backup"
MLX_PLIST="$HOME/Library/LaunchAgents/com.hanni.mlx-server.plist"
MODEL="mlx-community/Qwen3-32B-4bit"
LOG="$HANNI_DIR/training.log"
SYSTEM_PROMPT_FILE="$HANNI_DIR/system_prompt.txt"
MIN_FEEDBACK=10

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "$LOG"; }

log "=== Nightly training started ==="

# 1. Check if we have enough feedback (positive or negative)
FEEDBACK_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM message_feedback WHERE rating != 0 AND exported = 0;" 2>/dev/null || echo "0")
POSITIVE_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM message_feedback WHERE rating = 1 AND exported = 0;" 2>/dev/null || echo "0")
NEGATIVE_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM message_feedback WHERE rating = -1 AND exported = 0;" 2>/dev/null || echo "0")
log "Unexported feedback: $FEEDBACK_COUNT total ($POSITIVE_COUNT positive, $NEGATIVE_COUNT negative)"

if [ "$FEEDBACK_COUNT" -lt "$MIN_FEEDBACK" ]; then
    log "Not enough feedback ($FEEDBACK_COUNT < $MIN_FEEDBACK). Skipping."
    exit 0
fi

# 2. Export training data via sqlite3 + python
log "Exporting training data..."
mkdir -p "$TRAINING_DIR"

python3 << 'PYEOF'
import sqlite3, json, os

hanni_dir = os.path.expanduser("~/Library/Application Support/Hanni")
db_path = os.path.join(hanni_dir, "hanni.db")
training_dir = os.path.join(hanni_dir, "training")

conn = sqlite3.connect(db_path)

# Load feedback
feedback = {}
for cid, midx, rating in conn.execute("SELECT conversation_id, message_index, rating FROM message_feedback"):
    feedback.setdefault(cid, {})[midx] = rating

# Load conversations
rows = conn.execute(
    "SELECT id, messages FROM conversations WHERE message_count >= 4 ORDER BY started_at"
).fetchall()

# Read actual system prompt saved by Hanni at startup
prompt_file = os.path.join(hanni_dir, "system_prompt.txt")
if os.path.exists(prompt_file):
    SYSTEM_PROMPT = open(prompt_file).read()
else:
    SYSTEM_PROMPT = "You are Hanni, a personal AI assistant."
    print("WARNING: system_prompt.txt not found, using fallback")

rated = []
unrated = []

for conv_id, messages_json in rows:
    try:
        messages = json.loads(messages_json)
    except:
        continue

    ratings = feedback.get(conv_id, {})
    has_positive = any(v == 1 for v in ratings.values())

    real = [(r, c) for r, c in messages
            if r in ("user", "assistant")
            and not c.startswith("[Action result:")
            and "```action" not in c]
    if len(real) < 2:
        continue

    chat_msgs = [{"role": "system", "content": SYSTEM_PROMPT}]
    for idx, (role, content) in enumerate(messages):
        if role in ("user", "assistant"):
            # Skip negatively rated assistant messages
            if role == "assistant" and ratings.get(idx) == -1:
                continue
            clean = content.rstrip(" /no_think")
            chat_msgs.append({"role": role, "content": clean})

    example = {"messages": chat_msgs}
    if has_positive:
        rated.append(example)
    else:
        unrated.append(example)

all_examples = rated + unrated
if not all_examples:
    print("No training examples")
    exit(1)

total = len(all_examples)
train_end = int(total * 0.8) or 1
valid_end = train_end + max(int(total * 0.1), 1)

for name, data in [("train.jsonl", all_examples[:train_end]),
                    ("valid.jsonl", all_examples[train_end:valid_end]),
                    ("test.jsonl", all_examples[valid_end:])]:
    path = os.path.join(training_dir, name)
    with open(path, "w") as f:
        for ex in data:
            f.write(json.dumps(ex, ensure_ascii=False) + "\n")

print(f"Exported: {train_end} train, {valid_end - train_end} valid, {total - valid_end} test")
conn.close()
PYEOF

if [ $? -ne 0 ]; then
    log "Export failed"
    exit 1
fi
log "Export complete"

# 3. Stop MLX server
log "Stopping MLX server..."
launchctl bootout "gui/$(id -u)" "$MLX_PLIST" 2>/dev/null || true
# Also kill any lingering mlx_lm processes
pkill -f "mlx_lm server" 2>/dev/null || true
sleep 3
log "MLX server stopped"

# 4. Backup previous adapter
if [ -d "$ADAPTER_DIR" ]; then
    log "Backing up existing adapter..."
    rm -rf "$ADAPTER_BACKUP"
    cp -r "$ADAPTER_DIR" "$ADAPTER_BACKUP"
fi

# 5. Run LoRA fine-tuning
log "Starting LoRA training..."
TRAIN_START=$(date +%s)

python3 -m mlx_lm lora \
    --model "$MODEL" \
    --data "$TRAINING_DIR" \
    --adapter-path "$ADAPTER_DIR" \
    --mask-prompt \
    --batch-size 4 \
    --iters 300 \
    --lora-rank 8 \
    --learning-rate 1e-5 \
    >> "$LOG" 2>&1

TRAIN_STATUS=$?
TRAIN_END=$(date +%s)
TRAIN_DURATION=$(( TRAIN_END - TRAIN_START ))
log "Training finished in ${TRAIN_DURATION}s (exit code: $TRAIN_STATUS)"

if [ $TRAIN_STATUS -ne 0 ]; then
    log "Training FAILED. Restoring backup adapter..."
    if [ -d "$ADAPTER_BACKUP" ]; then
        rm -rf "$ADAPTER_DIR"
        mv "$ADAPTER_BACKUP" "$ADAPTER_DIR"
        log "Backup restored"
    else
        rm -rf "$ADAPTER_DIR"
        log "No backup to restore — will run without adapter"
    fi
fi

# 6. Mark feedback as exported
if [ $TRAIN_STATUS -eq 0 ]; then
    sqlite3 "$DB_PATH" "UPDATE message_feedback SET exported = 1 WHERE exported = 0;"
    log "Feedback marked as exported"
fi

# 7. Update plist to include adapter-path if adapter exists
if [ -f "$ADAPTER_DIR/adapters.safetensors" ]; then
    # Check if --adapter-path already in plist
    if ! grep -q "adapter-path" "$MLX_PLIST" 2>/dev/null; then
        log "Adding --adapter-path to MLX plist..."
        # Insert adapter args before </array> in ProgramArguments
        # Count existing args to append at the right index
        ARG_COUNT=$(/usr/libexec/PlistBuddy -c "Print :ProgramArguments" "$MLX_PLIST" 2>/dev/null | grep -c "^    " || echo "8")
        /usr/libexec/PlistBuddy -c "Add :ProgramArguments:${ARG_COUNT} string '--adapter-path'" "$MLX_PLIST" 2>/dev/null || true
        /usr/libexec/PlistBuddy -c "Add :ProgramArguments:$((ARG_COUNT + 1)) string '$ADAPTER_DIR'" "$MLX_PLIST" 2>/dev/null || true
        log "Plist updated with adapter path"
    fi
fi

# 8. Restart MLX server
log "Restarting MLX server..."
launchctl bootstrap "gui/$(id -u)" "$MLX_PLIST" 2>/dev/null || true
sleep 5

# Verify server is up
if curl -s --max-time 10 "http://127.0.0.1:8234/v1/models" > /dev/null 2>&1; then
    log "MLX server restarted successfully (with adapter: $([ -f "$ADAPTER_DIR/adapters.safetensors" ] && echo yes || echo no))"
else
    log "WARNING: MLX server did not respond after restart. May need manual intervention."
fi

log "=== Nightly training complete ==="
