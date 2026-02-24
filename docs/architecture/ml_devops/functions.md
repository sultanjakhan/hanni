# Module 15: ML/DevOps — Function Index

## Scripts and Configs

| ID | Function Group                              | Type      | Key Entry Point              | Location                   | LOC | Complexity | Subfuncs | Phase 3 |
|----|---------------------------------------------|-----------|------------------------------|----------------------------|-----|------------|----------|---------|
| B1 | Training data generation (77+ examples)     | script    | `generate_training_data.py`  | scripts/:L1-497            | 497 | Complex    | 7        | ✅      |
| B2 | Claude distillation (extract + refine)      | script    | `claude_distill.sh`          | scripts/:L1-274            | 274 | Complex    | 8        | ✅      |
| B3 | Nightly LoRA fine-tuning                    | scheduled | `nightly_train.sh`           | scripts/:L1-217            | 217 | Complex    | 9        | ✅      |
| B4 | LaunchAgent for nightly training            | config    | `com.hanni.nightly-train.plist` | scripts/                | ~30 | Simple     | —        | —       |
| B5 | CI/CD Release pipeline                      | workflow  | `release.yml`                | .github/workflows/:L1-65   | 65  | Medium     | 5        | ✅      |
| B6 | QLoRA fine-tuning скрипт                    | script    | `finetune.py`                | desktop/finetune.py          | 95  | Medium     | 3        | ✅      |
| B7 | Статус LoRA адаптера                        | endpoint  | `get_adapter_status()`       | lib.rs:L3026-3042            | 17  | Simple     | —        | —       |
| B8 | Запуск fine-tuning из UI                    | endpoint  | `run_finetune()`             | lib.rs:L3043-3083            | 41  | Medium     | —        | ✅      |

## Summary

- **Total functions**: 8
- **Simple**: 2 (B4, B7)
- **Medium**: 4 (B5, B6, B8)
- **Complex**: 3 (B1, B2, B3)
- **Total subfunctions**: 32
