# Module 15: ML/DevOps

## Description

Training scripts (data generation, Claude distillation, nightly LoRA fine-tuning), CI/CD release pipeline (GitHub Actions), and scheduling configs (LaunchAgent). Bridges the gap between the running app and the model improvement loop.

## Overview

| Attribute        | Value                                                                          |
|------------------|--------------------------------------------------------------------------------|
| Domain           | ML training, CI/CD, release pipeline                                           |
| Total LOC        | ~1180                                                                          |
| Scripts          | `scripts/generate_training_data.py`, `scripts/claude_distill.sh`, `scripts/nightly_train.sh` |
| Config           | `scripts/com.hanni.nightly-train.plist`, `.github/workflows/release.yml`      |
| Functions        | 5                                                                              |
| Complexity       | Simple: 1, Medium: 1, Complex: 3                                              |

## Files

| File                                          | Lines   | Role                                |
|-----------------------------------------------|---------|-------------------------------------|
| `scripts/generate_training_data.py`           | ~497    | Training data generation (77+ examples) |
| `scripts/claude_distill.sh`                   | ~274    | Claude-based distillation pipeline   |
| `scripts/nightly_train.sh`                    | ~217    | Nightly LoRA fine-tuning script      |
| `scripts/com.hanni.nightly-train.plist`       | ~30     | macOS LaunchAgent for scheduled training |
| `.github/workflows/release.yml`               | ~65     | CI/CD: build .app, publish release, update gist |

## Dependencies

| Direction | Module | Relationship                                          |
|-----------|--------|-------------------------------------------------------|
| Uses      | core   | DB access for exporting rated messages as training data |
| Used by   | voice  | LoRA adapter loaded by MLX server at runtime           |

## Key Concepts

- **Training Data Generation**: Python script producing 77+ example conversations from templates and DB-exported rated messages.
- **Claude Distillation**: Shell script that calls Claude API to extract high-quality responses, then refines them into training pairs.
- **Nightly LoRA Training**: Automated fine-tuning using `mlx_lm` LoRA support. Runs on schedule via LaunchAgent.
- **LaunchAgent**: `com.hanni.nightly-train.plist` schedules nightly execution on macOS.
- **Release Pipeline**: Push tag `vX.Y.Z` triggers GitHub Actions: build Tauri .app, create GitHub Release, update gist with `latest.json` for auto-updater.
- **Secrets Required**: `GITHUB_TOKEN`, `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `UPDATER_GITHUB_TOKEN`, `GIST_PAT`, `UPDATER_GIST_ID`.
