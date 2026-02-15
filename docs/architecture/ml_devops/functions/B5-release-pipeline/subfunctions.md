# B5 — CI/CD Release pipeline: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B5: CI/CD Release pipeline |
| Файл(ы) | .github/workflows/release.yml:L1-65 |
| LOC | 65 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

### Backend (Workflow)

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B5.1 | Триггер и настройка окружения | L1-28 | 28 | tag push -> runner config | CI: checkout, setup-node, rust-toolchain, rust-cache | control | Low |
| B5.2 | Установка системных зависимостей | L29-31 | 3 | -- -> cmake installed | CI: brew install cmake | external | Trivial |
| B5.3 | Установка npm зависимостей | L32-34 | 3 | -- -> node_modules | CI: npm install | external | Trivial |
| B5.4 | Сборка Tauri приложения и публикация Release | L36-49 | 14 | source -> .app + GitHub Release | CI: tauri-action build+sign+release; Secrets: 4 env vars | external | Medium |
| B5.5 | Обновление gist с latest.json | L51-65 | 15 | Release assets -> gist update | Network: gh api; File: download latest.json; External: gh gist edit | external | Medium |

## Data Flow

```
[Trigger: git push tag v*]
    │
    ▼
┌─ B5.1 Setup Environment ──────────────────┐
│  actions/checkout@v4                       │
│  actions/setup-node@v4 (node 20)           │
│  dtolnay/rust-toolchain@stable             │
│  Swatinem/rust-cache (desktop/src-tauri)   │
└──────┬────────────────────────────────────┘
       │
       ▼
┌─ B5.2-B5.3 Install Deps ──────────────────┐
│  brew install cmake (для whisper-rs-sys)   │
│  npm install (desktop/)                     │
└──────┬────────────────────────────────────┘
       │
       ▼
┌─ B5.4 Build & Release ────────────────────────────────┐
│  tauri-apps/tauri-action@v0                            │
│  env:                                                  │
│    GITHUB_TOKEN                                        │
│    TAURI_SIGNING_PRIVATE_KEY (code signing)             │
│    TAURI_SIGNING_PRIVATE_KEY_PASSWORD                   │
│    UPDATER_GITHUB_TOKEN (compiled into binary)          │
│  output:                                               │
│    .app bundle                                         │
│    GitHub Release (tag name, release name, assets)      │
│    latest.json (auto-updater manifest)                  │
└──────┬────────────────────────────────────────────────┘
       │ Release published
       ▼
┌─ B5.5 Update Gist ───────────────────────────────┐
│  TAG = github.ref_name                            │
│  gh api repos/.../releases/tags/$TAG              │ ←── [GitHub API]
│  Download latest.json from release assets         │
│  gh gist edit $GIST_ID -f latest.json             │ ──→ [GitHub Gist]
│  (using GIST_PAT for auth)                        │
│                                                    │
│  Gist URL -> Tauri updater checks this for updates│
└──────────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B5.4 | Build | Только macOS (macos-latest) — нет Windows/Linux билдов | Добавить матрицу OS когда понадобится multi-platform | Low |
| B5.5 | Gist update | Зависит от структуры release assets — если tauri-action изменит формат, сломается | Добавить валидацию latest.json перед обновлением gist | Low |
