# Tier 3 — owner-sync на GitHub private repo (E2E-encrypted store-and-forward)

Детальный дизайн замены Firestore-транспорта в [sync_owner.rs](../../desktop/src-tauri/src/sync_owner.rs).
Решение по backend'у принято (research-workflow `whg4n5noe`): **GitHub private repo** — free, без карты,
always-on, репо не засыпает/не удаляется по неактивности. Репо создан: `sultanjakhan/hanni-sync` (private).

**Инвариант:** merge-логика НЕ меняется. LWW по `_updated_at`, anti-resurrection по `sync_tombstones`,
special-case `event_categories` by name, курсор-семантика — переносятся 1:1. Меняется только транспорт
(`resolve_creds` / `patch_doc` / `run_query` / курсор pull) + добавляется E2E-обёртка.

---

## 1. Требование и почему GitHub

Async store-and-forward: устройство A пушит, устройство B забирает ПОЗЖЕ, даже если A офлайн (телефон
неделю выключен, Mac в этот момент тоже может быть выключен). Нужен бесплатный always-on durable
сторонний стор. GitHub: приватный репо хранится бессрочно, не засыпает, без карты; Hanni уже работает
с GitHub (`gh`, updater). Данные личные (health/money/contacts) → **стор видит только ciphertext**,
приватность репо — вторая линия.

## 2. Раскладка репозитория

Per-device **outbox**-поддеревья — каждое устройство пишет ТОЛЬКО в свою папку → git push-конфликтов нет.
Имена файлов **HMAC-хешированы** (скрывают имена таблиц/id); маршрутизация (`_table`, `id`) лежит ВНУТРИ
зашифрованного payload.

```
/<device_id>/<hmac(device_key, "row:{table}_{id}")>.bin        — последнее состояние строки (этим device)
/<device_id>/<hmac(device_key, "tomb:{table}_{id}")>.bin       — tombstone (deleted=1)
```

- `device_id` — уже есть в `app_settings` (стабильный UUID на установку, `device_id()` в sync_owner.rs:84).
- Имя файла детерминировано по `(table,id)` → повторная правка строки **перезаписывает тот же файл**
  (git видит modified → latest state). Echo-фильтр тривиален: устройство при pull **пропускает своё
  поддерево** `/<own_device_id>/`.
- `owner_uid` больше НЕ нужен для namespace (отдельный приватный репо = namespace сам по себе). →
  **Google sign-in становится не нужен** (Tier 1 держал `google_auth` только ради uid/project_id);
  Tier 3 сможет удалить `google_auth.rs` бонусом (см. §8).

## 3. E2E-encryption layer

- **Ключ.** Общий 32-байтный `device_key`, один на «owner» (оба устройства). Provisioning — §9.
- **Шифр.** XChaCha20-Poly1305 (AEAD). Per-blob random 24-byte nonce. `AAD = относительный путь файла`
  (привязка шифртекста к его doc-слоту — защита от подмены blob'ов местами).
- **Что шифруется.** Целиком результат `encode_doc()` (sync_owner.rs:136) — объект `{поля строки,
  _device_id, _updated_at, _synced_at, _table}`. (Firestore-специфичный `json_to_field` wrapper УБИРАЕТСЯ —
  шифруем чистый JSON, не Firestore field-format.)
- **Blob на диске:** `nonce || ciphertext || tag`, без base64 (файл бинарный, `.bin`). В Git Data API
  передаётся base64-ом в поле `content`.
- **Plaintext-метаданные:** только имя файла (HMAC-хеш — непрозрачно) и `device_id`-папка. Имя таблицы,
  id, значения — НЕ видны стору.
- **LWW** сравнивает `_updated_at` ПОСЛЕ расшифровки, на клиенте — как сейчас. Стор в merge не участвует.

Крейты: `chacha20poly1305` (XChaCha20), `hmac` + `sha2` (`sha2` уже в Cargo.toml), `rand` для nonce.

## 4. Push — батч-коммит через Git Data API

Сбор грязных строк — **как сейчас** (`push_table`: `updated_at > push_cursor` per-table, sync_owner.rs:245).
Отличие: вместо N×`patch_doc` — ОДИН коммит на весь sync через Git Data API (~4 вызова независимо от
числа строк, лимит 80/мин не достижим):

1. `GET /repos/{o}/hanni-sync/git/ref/heads/main` → текущий `commit_sha` + `tree_sha`.
2. Для каждой грязной строки/tombstone: `blob = encrypt(encode_doc(...))`; запись entry
   `{ path: "<device_id>/<hmac>.bin", mode: "100644", type: "blob", content: base64(blob) }`.
   (Tree API принимает inline `content` → блоб создаётся сервером; **один** вызов на все файлы.)
3. `POST /git/trees` с `base_tree=tree_sha` + все entries → `new_tree_sha`.
4. `POST /git/commits` с `tree=new_tree_sha`, `parents=[commit_sha]`, message → `new_commit_sha`.
5. `PATCH /git/refs/heads/main` `sha=new_commit_sha` → HEAD сдвинут.

Push-курсоры (`cloud_owner_v2_push_*`) — **без изменений** (per-table `updated_at`). Tombstones — те же,
но как зашифрованный blob с `_deleted` в payload (НЕ полагаемся на git-удаление файла — история его хранит).

## 5. Pull — diff с последнего обработанного commit SHA

Pull-курсор меняется с `_synced_at`-timestamp на **последний обработанный commit SHA**
(новый ключ `cloud_owner_gh_pull_sha`).

1. `GET /git/ref/heads/main` → `head_sha`. Если `head_sha == cursor_sha` → ничего нового, выходим.
2. `GET /repos/{o}/hanni-sync/compare/{cursor_sha}...{head_sha}` → `files[]` (filename + blob `sha`).
   Первый pull (курсор пуст): читаем всё дерево `GET /git/trees/{head_sha}?recursive=1`.
3. Для каждого изменённого файла **НЕ из своего** `device_id`-поддерева: `GET /git/blobs/{sha}` →
   base64-decode → `decrypt(AAD=path)` → JSON → применяем существующей логикой:
   - обычная строка → `upsert_row()` (LWW, anti-resurrection) — sync_owner.rs:324, БЕЗ изменений;
   - `_table=="tombstones"` → DELETE + tombstone-учёт — как в `pull_all` сейчас.
4. Сдвигаем курсор: `cloud_owner_gh_pull_sha = head_sha`.

~K+2 вызова на pull (K изменённых строк). При 2 устройствах низковолюмно.

## 6. Маппинг на `sync_owner.rs`

| Сейчас (Firestore) | Становится (GitHub) | НЕ меняется |
|---|---|---|
| `resolve_creds → (token, owner_uid, project_id)` (:39) | `resolve_gh → (pat, repo, device_key)` из `app_settings`. **Новый PAT**, не `UPDATER_GITHUB_TOKEN`. | контракт «креды из БД» |
| `encode_doc` оборачивает в `json_to_field` (Firestore) (:136,:154) | `encode_doc` отдаёт чистый JSON → `encrypt()` | поля `_device_id`/`_updated_at`/`_synced_at`/`_table` |
| `patch_doc` PUT одного документа (:181) | вклад в один батч-commit (tree entry) | — |
| `run_query` `_synced_at > since` (:197) | `compare(cursor_sha...head)` → changed blobs | — |
| `pull_all` курсор `cloud_owner_v2_pull_synced` (:483) | курсор `cloud_owner_gh_pull_sha` (commit SHA) | echo-filter, tombstone-DELETE, `upsert_row` |
| `decode_doc`/`decode_field` (Firestore field-format) (:158) | `decrypt()` + `serde_json::from` | — |
| `debug_owner_list` (:583) | переписать под `GET /contents` или удалить | — |

`encode_doc`, `decode→upsert_row`, LWW, `sync_tombstones` anti-resurrection, `event_categories`-by-name,
per-table push-курсоры, EPOCH-старт (первый push = полный backfill) — **переносятся как есть**.

## 7. Cutover — feature-flag, параллельно

Новый setting `cloud_owner_backend` ∈ `{firestore, github}` (default `firestore` до проверки).
`push_inner`/`pull_inner` (:534,:555) ветвятся по нему; `sync_owner_auto.rs` (фоновый цикл) — без изменений.

1. Реализуем GitHub-путь рядом с Firestore-путём (оба компилируются).
2. Флипаем флаг на `github` на Mac и Android, проверяем вживую: правка на Mac → видна на Android;
   тест офлайн-догона (Android выключить, поправить на Mac, включить Android — подтянул).
3. После проверки — удаляем Firestore-путь (`firestore_host`/`get_access_token`/`json_to_field` в
   `sync_share.rs` остаются, пока их использует Stage-A cloud-share; если cloud-share тоже снимается —
   отдельный шаг) + удаляем `google_auth.rs` (§8) + `jsonwebtoken` если больше не нужен.

Backfill отдельной миграции НЕ требует: новые github-push-курсоры стартуют с EPOCH → первый push шлёт
все строки; второе устройство забирает их первым pull (полное дерево).

## 8. Бонус: удаление Google sign-in

GitHub-backend не использует `owner_uid`/`project_id` → после cutover можно удалить весь `google_auth.rs`
(Tier 1 оставил его ТОЛЬКО ради этих двух полей), снять команды `google_auth_*` из `lib.rs`, убрать
`signInWithIdp`, `handle_oauth_callback`, OAuth-роут в `commands_meta.rs`, и UI Google-входа в
`cloud-share-modal.js`. Это закрывает последнюю Firebase/Google-зависимость. (Делать ПОСЛЕ успешного
cutover, не раньше.)

## 9. Provisioning (ключ + токен)

- **`device_key`** (общий E2E-ключ): сгенерить на первом устройстве (32 random байта), передать на второе
  существующим LAN-pairing каналом (QR/код, как `lan_sync_key`). **Хранение — открытый вопрос §11.**
- **PAT**: fine-grained, `Contents: read/write` ТОЛЬКО на `sultanjakhan/hanni-sync`. Генерит пользователь
  (это действие в GitHub UI), вставляет в Hanni-настройку → `app_settings.cloud_owner_gh_pat`.
- **repo**: `app_settings.cloud_owner_gh_repo = "sultanjakhan/hanni-sync"`.

## 10. Effort

**M.** Разбивка:
- E2E AEAD-модуль (новый файл `sync_crypto.rs`: encrypt/decrypt/hmac-имя + юнит-тесты) — **S–M**, без сети.
- GitHub-транспорт (`resolve_gh`, батч-commit push, compare-pull, новый pull-курсор) — **M**, reqwest уже есть.
- Feature-flag ветвление + настройки (PAT/repo/key/backend) + минимальный UI-ввод PAT — **S**.
- Cutover-проверка + удаление Firestore-пути + Google-auth (§8) — **S–M**, отдельным шагом после теста.

## 11. Открытые вопросы (нужны ДО кода)

**РЕШЕНО 2026-05-31:** (1) `device_key` → **app_settings** (v1, паритет с `service_account_json`; Keychain — follow-up);
(2) Google sign-in → **удалить после успешного cutover** (§8); (3) имена файлов → **HMAC-хеш**; (4) рост истории →
**принять, squash отложить**. Первый инкремент (E2E `sync_crypto.rs`) реализован.

1. **Хранение `device_key`** — OS Keychain (Mac) / Keystore (Android) [секьюрнее, но доступ из Rust на
   Android нетривиален], ИЛИ `app_settings` SQLite [консистентно с тем, как СЕЙЧАС лежит
   `service_account_json` в `cloud_share_config`; проще; менее секьюрно]. Рекомендация для v1: app_settings
   (паритет с текущим), Keychain — follow-up.
2. **Удалять ли Google sign-in** в этом же Tier 3 (§8) сразу после cutover, или оставить на потом?
3. **Имена файлов** — HMAC-хеш (скрывает таблицы, рекомендую) или plaintext `{table}_{id}` (проще
   дебажить приватный репо)?
4. **Рост истории** — для v1 принять неограниченный рост (squash later), или сразу snapshot-compaction?
   Рекомендация: принять, squash отложить.

## 12. Verification

- `sync_crypto.rs` — юнит-тесты (encrypt→decrypt round-trip, tamper→fail, nonce-uniqueness).
- `UPDATER_GITHUB_TOKEN=dummy cargo check` после каждого шага.
- Live Mac↔Android по флагу `github`: (a) правка на Mac ≤ интервал видна на Android; (b) **офлайн-догон**:
  Android выключить, изменить на Mac (Mac можно выключить после push), включить Android → подтянул; (c)
  удаление строки на одном → исчезает на другом (tombstone); (d) одновременная правка одной строки →
  LWW-сходимость к позднейшему `_updated_at`.
- Репо-инспекция: файлы в `hanni-sync` — бинарные `.bin`, имена непрозрачны, plaintext данных нет.
