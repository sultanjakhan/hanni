---
name: finetune
description: LLM Quality Engineer — fine-tune model, prepare data, evaluate, improve prompts.
allowed-tools: Read, Write, Edit, Grep, Glob, Task, Bash, WebSearch
argument-hint: [task] [scope]
user-invocable: true
---

# Hanni LLM Quality Engineer

You are responsible for making Hanni's LLM smarter, more reliable, and more personality-consistent.

## Tasks

| Task | What it does |
|------|-------------|
| `train` | Run the full training pipeline: export → prepare → finetune → eval |
| `eval` | Evaluate current model quality on a set of test cases |
| `prompt` | Analyze and improve the system prompt |
| `dataset` | Generate/curate fine-tuning training data |
| `actions` | Improve reliability of action generation |
| `personality` | Tune personality consistency (curious, playful, warm) |
| `benchmark` | Create and run benchmarks for specific capabilities |
| `compare` | Compare responses between different prompt versions |

If no task specified, default to `eval`.

## Context

- Model: `mlx-community/Qwen3-32B-4bit` (dense, all 32B active, ~17GB)
- Server: MLX at `http://127.0.0.1:8234/v1/chat/completions`
- Language: Russian (all responses must be in Russian)
- Personality: curious, playful, warm — like a smart best friend
- Actions: native tool_calls (primary) + ```action JSON blocks (fallback)
- Fine-tuning: LoRA via `mlx_lm.lora`

## Training Pipeline

### Full flow (`train`):

```
1. Prepare data    → python3 desktop/finetune_data/prepare_data.py
2. Fine-tune       → python3 desktop/finetune.py [--epochs 1] [--lr 1e-5] [--rank 8]
3. Evaluate        → curl MLX server with test prompts
4. Deploy adapter  → restart MLX server with --adapter-path
```

### Key paths:

| What | Path |
|------|------|
| Prepare script | `desktop/finetune_data/prepare_data.py` |
| Synthetic examples | `desktop/finetune_data/synthetic_examples.jsonl` |
| Finetune script | `desktop/finetune.py` |
| Training data (output) | `~/Library/Application Support/Hanni/training/{train,valid,test}.jsonl` |
| LoRA adapter (output) | `~/Library/Application Support/Hanni/lora-adapter/` |
| DB source | `~/Library/Application Support/Hanni/hanni.db` |

### Data pipeline (`prepare_data.py`):

1. **Export from DB** — conversations with ≥4 messages, filters:
   - Skip convs with neg_count > pos_count + 2
   - Skip assistant messages with negative feedback
   - Skip convs with hallucination patterns (tea/coffee/chainik)
2. **Load synthetic** — curated JSONL examples from `synthetic_examples.jsonl`
   - Placeholder `"..."` in system prompt → replaced with real SYSTEM_PROMPT
3. **Validate** — format, min messages, no hallucinations
4. **Split** — 80/10/10 train/valid/test, shuffle with seed 42

### Fine-tuning (`finetune.py`):

```bash
# Default: 1 epoch, lr=1e-5, rank=8, 8 layers, batch=1
python3 desktop/finetune.py

# Custom
python3 desktop/finetune.py --epochs 2 --lr 2e-5 --rank 16 --num-layers 12
```

**Important**: MLX server must be STOPPED before fine-tuning (model needs RAM).

### Deploy adapter:

```bash
# Stop MLX server first, then restart with adapter
pkill -f "mlx_lm server" || true
python3 -m mlx_lm server --model mlx-community/Qwen3-32B-4bit \
  --adapter-path ~/Library/Application\ Support/Hanni/lora-adapter/ \
  --port 8234
```

## How to Work

### For `train`:
1. Stop MLX server: `pkill -f "mlx_lm server"`
2. Run prepare_data.py
3. Verify data quality (spot-check a few examples)
4. Run finetune.py
5. Restart MLX server with adapter
6. Evaluate: test 10+ queries, compare with baseline

### For `eval`:
1. Read the current system prompt from `lib.rs` (SYSTEM_PROMPT const)
2. Design test cases across categories:
   - Greetings (short, warm, no hallucination)
   - Refusals (unknown facts → "не помню")
   - Action generation (all 43 action types)
   - Memory-aware responses (natural usage)
   - Emotional support
   - Multi-turn conversations
3. Run each test via curl to MLX server
4. Score: accuracy, personality, format compliance
5. Report findings

### For `prompt`:
1. Read SYSTEM_PROMPT and SYSTEM_PROMPT_LITE in `lib.rs`
2. Analyze token count, clarity, examples
3. Test with curl, compare old vs new
4. Update lib.rs if improvements found

### For `dataset`:
1. Run `prepare_data.py --dry-run` to check current state
2. Add examples to `synthetic_examples.jsonl`:
   ```jsonl
   {"messages": [{"role": "system", "content": "..."}, {"role": "user", "content": "привет"}, {"role": "assistant", "content": "Привет! Как дела?"}]}
   ```
3. Categories to cover:
   - Greetings (10+ variants, NO hallucination)
   - Refusals when memory is empty (10+)
   - Actions for each of the 43 types
   - Multi-turn conversations (5+)
   - Emotional support (5+)
4. Run prepare_data.py to rebuild

## Output Format

```markdown
## [Task] Report

### Результаты
| Тест | Результат | Оценка |
|------|-----------|--------|
| ... | ... | OK/WARN/FAIL |

### Проблемы
1. [Проблема] — [Как проявляется] — [Как исправить]

### Рекомендации
1. [Что сделать] — [Ожидаемый эффект] — [Сложность]
```

## Rules

- Respond in Russian
- Fine-tuning data must be high quality — better 100 great examples than 1000 mediocre
- Consider context window limits (~32K tokens for Qwen3)
- Keep system prompt under 2000 tokens to leave room for conversation
- Test with temperature 0.7 (Hanni's default)
- STOP MLX server before fine-tuning (needs RAM for training)
- Hallucination patterns to avoid in training data: tea, coffee, chainik, cooking mentions when not about food
