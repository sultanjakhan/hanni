#!/usr/bin/env python3
"""
Hanni Fine-tuning — QLoRA adapter training via mlx_lm.lora.

Usage:
    python3 finetune.py [--epochs 1] [--lr 1e-5] [--rank 8]

Data:   ~/Library/Application Support/Hanni/training/{train,valid}.jsonl
Output: ~/Library/Application Support/Hanni/lora-adapter/
"""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

DATA_DIR = Path.home() / "Library" / "Application Support" / "Hanni" / "training"
ADAPTER_DIR = Path.home() / "Library" / "Application Support" / "Hanni" / "lora-adapter"
MODEL = "mlx-community/Qwen3-32B-4bit"


def check_data():
    train = DATA_DIR / "train.jsonl"
    valid = DATA_DIR / "valid.jsonl"
    if not train.exists():
        print(f"ERROR: {train} not found. Export training data from Hanni first.")
        sys.exit(1)
    if not valid.exists():
        print(f"ERROR: {valid} not found. Export training data from Hanni first.")
        sys.exit(1)

    train_count = sum(1 for _ in open(train))
    valid_count = sum(1 for _ in open(valid))
    print(f"Training data: {train_count} train, {valid_count} valid examples")

    if train_count < 5:
        print("WARNING: Very few training examples. Results may be poor.")

    return train_count, valid_count


def run_finetune(epochs=1, lr=1e-5, rank=8, num_layers=8, batch_size=1):
    check_data()

    ADAPTER_DIR.mkdir(parents=True, exist_ok=True)

    cmd = [
        sys.executable, "-m", "mlx_lm.lora",
        "--model", MODEL,
        "--data", str(DATA_DIR),
        "--adapter-path", str(ADAPTER_DIR),
        "--train",
        "--iters", str(epochs * 100),  # approximate: ~100 iters per epoch
        "--learning-rate", str(lr),
        "--lora-rank", str(rank),
        "--lora-layers", str(num_layers),
        "--batch-size", str(batch_size),
        "--val-batches", "5",
        "--save-every", "50",
    ]

    print(f"Running: {' '.join(cmd)}")
    print(f"Adapter will be saved to: {ADAPTER_DIR}")
    print("---")

    result = subprocess.run(cmd, cwd=str(Path.home()))

    if result.returncode == 0:
        # Write metadata
        meta = {
            "model": MODEL,
            "epochs": epochs,
            "lr": lr,
            "rank": rank,
            "num_layers": num_layers,
            "trained_at": __import__("datetime").datetime.now().isoformat(),
        }
        with open(ADAPTER_DIR / "hanni_meta.json", "w") as f:
            json.dump(meta, f, indent=2)
        print("\nDone! Adapter saved. Restart MLX server to use it.")
    else:
        print(f"\nFine-tuning failed with exit code {result.returncode}")
        sys.exit(result.returncode)


def main():
    parser = argparse.ArgumentParser(description="Hanni Fine-tuning (QLoRA via mlx_lm)")
    parser.add_argument("--epochs", type=int, default=1, help="Number of epochs (default: 1)")
    parser.add_argument("--lr", type=float, default=1e-5, help="Learning rate (default: 1e-5)")
    parser.add_argument("--rank", type=int, default=8, help="LoRA rank (default: 8)")
    parser.add_argument("--num-layers", type=int, default=8, help="Number of LoRA layers (default: 8)")
    parser.add_argument("--batch-size", type=int, default=1, help="Batch size (default: 1)")
    args = parser.parse_args()

    run_finetune(
        epochs=args.epochs,
        lr=args.lr,
        rank=args.rank,
        num_layers=args.num_layers,
        batch_size=args.batch_size,
    )


if __name__ == "__main__":
    main()
