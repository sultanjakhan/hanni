"""
Fine-tuning script for GLM-4.7 using MLX-LM
Использует собранные данные для дообучения модели
"""
import json
import argparse
from pathlib import Path

# Training data directory
DATA_DIR = Path(__file__).parent / "training_data"
MODEL_DIR = Path.home() / ".lmstudio/models/mlx-community/GLM-4.7-Flash-6bit"
OUTPUT_DIR = Path(__file__).parent / "finetuned_models"


def prepare_dataset(input_file: str = "training_dataset.jsonl", output_file: str = "train.jsonl"):
    """Подготовка датасета в формате для MLX-LM"""

    input_path = DATA_DIR / input_file
    output_path = DATA_DIR / output_file

    if not input_path.exists():
        print(f"Error: {input_path} not found")
        print("Run /export in the bot first to create training data")
        return None

    entries = []
    with open(input_path, "r") as f:
        for line in f:
            data = json.loads(line)
            # MLX-LM format
            entry = {
                "text": format_conversation(data["messages"])
            }
            entries.append(entry)

    # Split into train/valid (90/10)
    split_idx = int(len(entries) * 0.9)
    train_entries = entries[:split_idx]
    valid_entries = entries[split_idx:]

    # Save train
    train_path = DATA_DIR / "train.jsonl"
    with open(train_path, "w") as f:
        for entry in train_entries:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")

    # Save valid
    valid_path = DATA_DIR / "valid.jsonl"
    with open(valid_path, "w") as f:
        for entry in valid_entries:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")

    print(f"Prepared {len(train_entries)} training examples")
    print(f"Prepared {len(valid_entries)} validation examples")

    return train_path, valid_path


def format_conversation(messages: list) -> str:
    """Форматирует разговор в текст для обучения"""
    text = ""
    for msg in messages:
        role = msg["role"]
        content = msg["content"]
        if role == "system":
            text += f"<|system|>\n{content}\n"
        elif role == "user":
            text += f"<|user|>\n{content}\n"
        elif role == "assistant":
            text += f"<|assistant|>\n{content}\n"
    return text.strip()


def finetune(
    epochs: int = 3,
    batch_size: int = 1,
    learning_rate: float = 1e-5,
    lora_rank: int = 8
):
    """Запуск fine-tuning с LoRA через MLX-LM"""

    train_path = DATA_DIR / "train.jsonl"
    valid_path = DATA_DIR / "valid.jsonl"

    if not train_path.exists():
        print("Preparing dataset first...")
        result = prepare_dataset()
        if not result:
            return

    OUTPUT_DIR.mkdir(exist_ok=True)
    adapter_path = OUTPUT_DIR / "glm-4.7-lora-adapter"

    print(f"""
Fine-tuning Configuration:
- Model: {MODEL_DIR}
- Train data: {train_path}
- Valid data: {valid_path}
- Output: {adapter_path}
- Epochs: {epochs}
- Batch size: {batch_size}
- Learning rate: {learning_rate}
- LoRA rank: {lora_rank}
""")

    # MLX-LM fine-tuning command
    cmd = f"""
mlx_lm.lora \\
    --model {MODEL_DIR} \\
    --train \\
    --data {DATA_DIR} \\
    --adapter-path {adapter_path} \\
    --iters {epochs * 100} \\
    --batch-size {batch_size} \\
    --learning-rate {learning_rate} \\
    --lora-rank {lora_rank} \\
    --lora-layers 16
"""

    print("Run this command to start fine-tuning:")
    print(cmd)

    return cmd


def merge_adapter(adapter_path: str = None):
    """Объединяет LoRA адаптер с базовой моделью"""

    if adapter_path is None:
        adapter_path = OUTPUT_DIR / "glm-4.7-lora-adapter"

    merged_path = OUTPUT_DIR / "glm-4.7-finetuned"

    cmd = f"""
mlx_lm.fuse \\
    --model {MODEL_DIR} \\
    --adapter-path {adapter_path} \\
    --save-path {merged_path}
"""

    print("Run this command to merge adapter:")
    print(cmd)

    return cmd


def check_data_stats():
    """Показывает статистику собранных данных"""

    files = {
        "all_conversations.jsonl": "Total conversations",
        "good_examples.jsonl": "Good examples",
        "corrections.jsonl": "Corrections",
        "training_dataset.jsonl": "Ready for training"
    }

    print("\nTraining Data Statistics:")
    print("-" * 40)

    for filename, desc in files.items():
        filepath = DATA_DIR / filename
        if filepath.exists():
            with open(filepath, "r") as f:
                count = sum(1 for _ in f)
            print(f"{desc}: {count}")
        else:
            print(f"{desc}: 0 (file not created yet)")

    print("-" * 40)


def main():
    parser = argparse.ArgumentParser(description="Fine-tune GLM-4.7")
    parser.add_argument("--prepare", action="store_true", help="Prepare dataset")
    parser.add_argument("--train", action="store_true", help="Show training command")
    parser.add_argument("--merge", action="store_true", help="Show merge command")
    parser.add_argument("--stats", action="store_true", help="Show data statistics")
    parser.add_argument("--epochs", type=int, default=3, help="Number of epochs")
    parser.add_argument("--lr", type=float, default=1e-5, help="Learning rate")
    parser.add_argument("--rank", type=int, default=8, help="LoRA rank")

    args = parser.parse_args()

    if args.stats:
        check_data_stats()
    elif args.prepare:
        prepare_dataset()
    elif args.train:
        finetune(epochs=args.epochs, learning_rate=args.lr, lora_rank=args.rank)
    elif args.merge:
        merge_adapter()
    else:
        # Default: show stats and help
        check_data_stats()
        print("\nUsage:")
        print("  python finetune.py --stats    # Show data statistics")
        print("  python finetune.py --prepare  # Prepare dataset for training")
        print("  python finetune.py --train    # Show training command")
        print("  python finetune.py --merge    # Show merge command")


if __name__ == "__main__":
    main()
