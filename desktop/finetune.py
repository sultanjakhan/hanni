#!/usr/bin/env python3
"""
Hanni Fine-tuning — LoRA adapter training via mlx_lm.

Usage:
    python3 finetune.py [--epochs 2] [--lr 2e-5] [--rank 8]

Data:   ~/Library/Application Support/Hanni/training/{train,valid}.jsonl
Output: ~/Library/Application Support/Hanni/lora-adapter/
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
import yaml
from pathlib import Path

DATA_DIR = Path.home() / "Library" / "Application Support" / "Hanni" / "training"
ADAPTER_DIR = Path.home() / "Library" / "Application Support" / "Hanni" / "lora-adapter"
MODEL = "NexVeridian/Qwen3.5-35B-A3B-4bit"


def check_data():
    train = DATA_DIR / "train.jsonl"
    valid = DATA_DIR / "valid.jsonl"
    if not train.exists():
        print(f"ERROR: {train} not found. Run prepare_data.py first.")
        sys.exit(1)
    if not valid.exists():
        print(f"ERROR: {valid} not found. Run prepare_data.py first.")
        sys.exit(1)

    train_count = sum(1 for _ in open(train))
    valid_count = sum(1 for _ in open(valid))
    print(f"Training data: {train_count} train, {valid_count} valid examples")

    if train_count < 5:
        print("WARNING: Very few training examples. Results may be poor.")

    return train_count, valid_count


def run_finetune(epochs=2, lr=2e-5, rank=8, num_layers=16, batch_size=1, grad_checkpoint=False):
    train_count, _ = check_data()

    ADAPTER_DIR.mkdir(parents=True, exist_ok=True)

    # Calculate iterations: iters = epochs * train_count
    iters = epochs * train_count

    # Create config YAML for LoRA parameters
    # scale = alpha / rank (standard: alpha=2*rank → scale=2.0)
    config = {
        "lora_parameters": {
            "rank": rank,
            "scale": 2.0,
            "dropout": 0.05,
        }
    }

    # Write temp config file
    config_path = ADAPTER_DIR / "lora_config.yaml"
    with open(config_path, "w") as f:
        yaml.dump(config, f)

    # Use new subcommand syntax: python3 -m mlx_lm lora
    cmd = [
        sys.executable, "-m", "mlx_lm", "lora",
        "--model", MODEL,
        "--data", str(DATA_DIR),
        "--adapter-path", str(ADAPTER_DIR),
        "--train",
        "--fine-tune-type", "lora",
        "--iters", str(iters),
        "--learning-rate", str(lr),
        "--num-layers", str(num_layers),
        "--batch-size", str(batch_size),
        "--val-batches", "5",
        "--steps-per-report", "10",
        "--steps-per-eval", "50",
        "--save-every", "50",
        "--max-seq-length", "2048",
        "-c", str(config_path),
    ]

    if grad_checkpoint:
        cmd.append("--grad-checkpoint")

    print(f"Running: {' '.join(cmd)}")
    print(f"Config: rank={rank}, alpha={rank*2}, layers={num_layers}, lr={lr}")
    print(f"Iterations: {iters} ({epochs} epochs x {train_count} examples)")
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
            "alpha": rank * 2,
            "num_layers": num_layers,
            "iters": iters,
            "train_count": train_count,
            "trained_at": __import__("datetime").datetime.now().isoformat(),
        }
        with open(ADAPTER_DIR / "hanni_meta.json", "w") as f:
            json.dump(meta, f, indent=2)
        print("\nDone! Adapter saved.")
        print(f"To use: python3 -m mlx_lm server --model {MODEL} --adapter-path {ADAPTER_DIR} --port 8234")
    else:
        print(f"\nFine-tuning failed with exit code {result.returncode}")
        sys.exit(result.returncode)


def main():
    parser = argparse.ArgumentParser(description="Hanni Fine-tuning (LoRA via mlx_lm)")
    parser.add_argument("--epochs", type=int, default=2, help="Number of epochs (default: 2)")
    parser.add_argument("--lr", type=float, default=2e-5, help="Learning rate (default: 2e-5)")
    parser.add_argument("--rank", type=int, default=8, help="LoRA rank (default: 8)")
    parser.add_argument("--num-layers", type=int, default=16, help="Number of LoRA layers (default: 16)")
    parser.add_argument("--batch-size", type=int, default=1, help="Batch size (default: 1)")
    parser.add_argument("--grad-checkpoint", action="store_true", help="Enable gradient checkpointing (saves RAM)")
    args = parser.parse_args()

    run_finetune(
        epochs=args.epochs,
        lr=args.lr,
        rank=args.rank,
        num_layers=args.num_layers,
        batch_size=args.batch_size,
        grad_checkpoint=args.grad_checkpoint,
    )


if __name__ == "__main__":
    main()
