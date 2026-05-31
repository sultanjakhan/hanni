# Firebase-off Migration Plan

> Финальный план поэтапного выноса Firebase/Firestore из Hanni. Tier 1 и Tier 2 — безопасные, не меняют поведение живых потоков. Tier 3 — единственная развилка, требующая твоего решения (см. §5). Все номера строк указаны на момент анализа; перед правкой сверяй `grep`/`rg`, файлы активно меняются.

## 1. Context

Firebase/Firestore исторически выполняет в Hanni три роли, которые анализ разнёс по трём «тирам» по степени связанности с живым поведением:

- **Tier 1 — мёртвый Firebase-auth хвост.** Модуль `google_auth.rs` документирует обмен Google id_token → Firebase id_token и refresh, плюс `firestore_admin.rs` (one-shot создание Firestore-базы и деплой security-rules). Эти пути **никем не читаются**: `sync_owner` берёт из Google-сессии только `session.uid` и `config.project_id`, а `firestore_setup` не имеет ни одного JS-вызова. Удаление = ноль изменений поведения.
- **Tier 2 — guest read-fallback через Firestore.** Когда Mac-хост офлайн, гостевые view-модули (`guest_recipes.js`, `guest_products.js`, и т. д.) молча читают `share_links/{token}/*` напрямую из Firestore REST. В текущей Tailscale/axum-сборке эта ветка **уже мертва** (`window.__SHARE__.tunnel_url` не инжектится → `haveTunnel` всегда false, origin никогда не `*.web.app` → `isStaticOrigin` всегда false). Удаление теряет ровно одну способность: read-резильентность при офлайн-хосте.
- **Tier 3 — load-bearing owner-sync.** `sync_owner.rs` + `sync_owner_auto.rs` зеркалят 62 таблицы Mac↔Android через Firestore REST с service-account JWT, курсорами, tombstones и LWW. Это **рабочий механизм**, его нельзя просто удалить — нужно заменить. Две опции (Tailscale-only / cloud-broker) разобраны в §5.

Ключевой инвариант на всех тирах: **owner-sync (Tier 3) аутентифицируется service-account JWT из `cloud_share_config`, а из `google_auth` читает только `uid` + `project_id`** — поэтому Tier 1 может вырезать Firebase-id-token хвост, не трогая sync.

---

## 2. Call-site inventory (по тирам)

### Tier 1 — dead Firebase-auth / firestore-admin (удаляется)

| file:line | what | removability |
|---|---|---|
| `google_auth.rs:1-14` | Doc-comment про Google→Firebase обмен и «get_firebase_id_token — единственная точка входа для sync» (ложь, вызывающих нет) | dead |
| `google_auth.rs:22` | `SETTING_CONFIG` — ключ `google_auth_config` | dead |
| `google_auth.rs:23` | `SETTING_SESSION` — ключ `google_auth_session` | dead |
| `google_auth.rs:31` | `GoogleAuthConfig.api_key` (Firebase Web API key) | dead |
| `google_auth.rs:36-37` | `firebase_id_token` / `firebase_refresh_token` поля сессии — пишутся, не читаются | dead |
| `google_auth.rs:41-44` | `google_access_token` (cloud-platform scope) — только для firestore_admin | dead |
| `google_auth.rs:136` | `google_auth_set_config` команда | dead |
| `google_auth.rs:163` | `oauth2.googleapis.com/revoke` POST на signout | dead |
| `google_auth.rs:191-193` | cloud-platform scope (только для firestore_admin) | dead |
| `google_auth.rs:234-271` | code-exchange + `signInWithIdp` REST (Firebase id_token mint) | dead* |
| `google_auth.rs:310-346` | `get_google_access_token` + refresh POST | dead |
| `google_auth.rs:350-388` | `get_firebase_id_token` + `securetoken.googleapis.com` POST — 0 вызовов | dead |
| `firestore_admin.rs:1-156` | весь модуль: `ensure_database`, `deploy_rules`, `RULES_TEXT`, `firestore_setup` | dead |
| `lib.rs:61` | `mod firestore_admin;` | dead |
| `lib.rs:862` | регистрация `firestore_admin::firestore_setup` | dead |
| `Cargo.toml:42` | `percent-encoding` comment про «Stage C.1 Google Auth» (сам crate KEEP — нужен vacancy.rs) | dead (comment) |
| `projects.yaml:703-706` | `_shared.security` запись про firestore_admin.rs | dead |

\* `signInWithIdp` — **условно dead**: см. unconfirmed в §3 (источник `uid`).

**KEEP (несмотря на попадание в Tier-1 список):** `google_auth.rs:30` `project_id`, `:74` `load_session`, `:70` `load_config`, `:120` `google_auth_status`, `:150` `google_auth_signout` (local wipe), `:185` `google_auth_start_signin`, `:213` `handle_oauth_callback`, `:302` `google-auth-changed` emit — все они питают `session.uid`/`project_id` для Tier-3 sync или UI входа.

### Tier 2 — guest Firestore read-fallback (удаляется)

| file:line | what | removability |
|---|---|---|
| `share_assets/guest_firestore.js:13-122` | весь Firestore REST-клиент (parse/cache/list/get, `window.HanniGuest.firestore`) | bypassed |
| `share_server.rs:151-156,165` | `landing()` инжектит `{{FIRESTORE}}` из `load_config` | bypassed |
| `share_server.rs:68` | route `/assets/guest_firestore.js` | bypassed |
| `share_static.rs:15-19` | `asset_js_firestore` + `include_str!` | bypassed |
| `share_assets/guest.html:19,33` | `firestore: {{FIRESTORE}}` + `<script guest_firestore.js>` | bypassed |
| `guest.js:4-7,16-24,39-45,126-134` | tunnel const, `isStaticOrigin`/`HOST_OFFLINE` guard, cache-invalidate, Hosting-комментарии | bypassed |
| `guest_recipes.js:48-97,215` | fs/haveTunnel, `fetchFromFirestore`, fallback в `fetchListAndCatalog`/`fetchRecipe` | bypassed |
| `guest_products.js:26-41` | fs/haveTunnel + `ingredient_catalog` fallback | bypassed |
| `guest_meal_plan.js:24-57` | fs/haveTunnel + `meal_plan`/`recipes` fallback | bypassed |
| `guest_memory.js:26-40` | fs/haveTunnel + `food_blacklist` fallback | bypassed |
| `guest_fridge.js:7-37` | fs/haveTunnel + `ingredient_catalog`/`products` fallback | bypassed |
| `share_server.rs:84-105` | CORS-предикаты `hanni-2e5d0.web.app` / `.firebaseapp.com` | bypassed |
| `Cargo.toml:32` | `tower-http` comment про «Firebase Hosting» (crate + `cors` feature KEEP) | bypassed (comment) |
| `commands_share.rs:36,109-112` | стейл-комментарии про Firebase Hosting/Firestore | bypassed (comment) |
| `sync_share.rs:503,647` | стейл-комментарии про «Firebase-Hosting guests read tunnel_url» | bypassed (comment) |
| `share-modal.js:25,246,261` | UI-лейблы про Firebase read-fallback | bypassed (comment/copy) |
| `projects.yaml:356,386` | manifest: guest_firestore.js, «Firebase Hosting fallback» | bypassed |

### Tier 3 — load-bearing owner-sync (заменяется, не удаляется напрямую)

| file:line | what | removability |
|---|---|---|
| `sync_owner.rs:1-611` | весь модуль: `resolve_creds`, `encode_doc`/`decode_doc`, `patch_doc`, `run_query`, `push_table`/`pull_all`/`push_tombstones`, `push_inner`/`pull_inner`, `cloud_owner_*` команды, LWW/tombstone/cursor логика | load-bearing |
| `sync_owner_auto.rs:1-108` | 3s background loop, `cloud_owner_set/get_auto`, 429-backoff | load-bearing |
| `sync_share.rs:3-703` | Stage A/C-1 push: `CloudShareConfig`, `get_access_token` (JWT), `json_to_field`, `firestore_upsert_snapshot`, dirty-queue `mark_dirty`/`mirror_pending` | load-bearing |
| `lan_sync.rs:14` | импорт `upsert_row`/`row_to_json`/`get_setting`/`set_setting` из sync_owner — **разделяемые хелперы** | load-bearing (relocate, не delete) |
| `Cargo.toml:39-40` | `jsonwebtoken`, `sha2` — service-account JWT + owner_uid derivation | load-bearing |
| `lib.rs:56-58,845-852,919-923` | `mod sync_share/sync_owner/sync_owner_auto`, регистрация команд, spawn loop | load-bearing |
| `cloud-share-modal.js:21,58` | UI про Firestore owner-sync / LAN contrast | load-bearing |
| `share-modal.js:85,143` | `cloud_share_push` invoke + кнопка ☁ Push | load-bearing |
| `state.js:3,17-23` + `sync-trigger.js:23` | central `invoke()` wrapper → `requestPush` → `cloud_owner_push` после каждого write | load-bearing (silent-break риск) |

---

## 3. Tier 1 — removal steps

**Behavior impact:** ноль для любого живого потока. Owner-sync продолжает работать без изменений (JWT из `cloud_share_config`, читает только `session.uid` + `project_id`). Единственная внутренняя разница: в `google_auth_session` больше не пишутся `firebase_id_token`/`firebase_refresh_token`/`google_access_token`/`google_refresh_token`/`google_expires_at` — serde игнорирует неизвестные ключи при чтении старых сессий, миграция/повторный вход не нужны.

1. **delete-file** `firestore_admin.rs` целиком. `firestore_setup` (единственная `#[tauri::command]`) не имеет JS-вызовов; `ensure_database`/`deploy_rules`/`RULES_*` внутренние; это единственный потребитель `get_google_access_token`.
   *Gate (после шага 3):* `UPDATER_GITHUB_TOKEN=dummy cargo check` — без unresolved-module/unused-import.
2. **lib.rs** — убрать строку `firestore_admin::firestore_setup,` из `invoke_handler!` (`:862`). Регистрации `google_auth::*` (858-861) **оставить**.
3. **lib.rs** — убрать `mod firestore_admin;` (`:61`). `mod google_auth;` (`:60`) **оставить**.
   *Gate:* нет `unresolved module firestore_admin`.
4. **google_auth.rs** — удалить `get_firebase_id_token()` (`:348-388` с doc-comment). 0 вызовов; единственный путь, читающий `firebase_refresh_token`/`firebase_id_token`, и единственный достижимый вызов `securetoken.googleapis.com`.
   *Gate:* `grep -rn get_firebase_id_token desktop/` пусто.
5. **google_auth.rs** — удалить `get_google_access_token()` (`:310-346`). После шага 1 его единственный вызывающий (firestore_admin) исчез. Убирает второй `oauth2.googleapis.com/token` refresh-путь.
   *Gate:* `grep -rn get_google_access_token desktop/` пусто.
6. **google_auth.rs** — из `GoogleAuthSession` (`:35-51`) удалить поля `firebase_id_token`, `firebase_refresh_token`, `google_access_token`, `google_refresh_token`, `google_expires_at` (+ doc-comment 41-44). Обновить единственное место конструирования (`:278` в `handle_oauth_callback`), убрав инициализаторы и теперь-неиспользуемые локалы (`google_access_token` 244-245, `google_refresh_token` 246-247, `google_expires_in` 248-249). **KEEP** `local_id`/`email`/`expires_at` (питают `uid`/`email`, которые читает sync).
   - ⚠️ **СТОП по signInWithIdp:** `handle_oauth_callback` сейчас выводит `uid` (`localId`) и `email` из ответа Firebase `signInWithIdp`, а **не** из Google id_token. Удаление `signInWithIdp` сменит источник `uid` → перекеит Firestore-путь каждого owner'а. **Не трогать `signInWithIdp` (252-271) в этом шаге** — см. unconfirmed ниже.
   *Gate:* `cargo check` — struct, serde round-trip и `handle_oauth_callback` компилируются без unused-var.
7. **google_auth.rs** — в `google_auth_signout` (`:150-181`) убрать best-effort `oauth2.googleapis.com/revoke` блок (`:157-169`) **только если** шаг 6 удалил `google_refresh_token`. Local-state wipe (del_setting SESSION/PENDING_STATE + reset sync-bookmark) **оставить**.
   *Gate:* нет ссылок на `s.google_refresh_token`; signout всё ещё чистит сессию.
8. **projects.yaml** — убрать `desktop/src-tauri/src/firestore_admin.rs` (`:706`) из `_shared.security.rust`; `google_auth.rs` (`:705`) оставить. Переписать `security.description` (`:703`), убрав «Firestore admin SDK». Тем же коммитом (manifest contract).
   *Gate:* `/audit-projects` — нет phantom firestore_admin.rs.
9. **google_auth.rs** — переписать module doc-comment (`:1-14`): убрать ложные claims про Firebase id_token exchange и «get_firebase_id_token — единственная точка входа». Зафиксировать реальную цель: Google OAuth sign-in → стабильный `uid` + `project_id` для `sync_owner`.
   *Gate:* `cargo check` (комментарий инертен).

**Tier-1 unconfirmed (требуют решения, но НЕ блокируют шаги 1-9):**
- **`signInWithIdp` как источник uid** — Firebase exchange сейчас даёт `session.uid`, от которого зависит Tier-3 sync-путь. Выглядит как Firebase-хвост, но удаление сменит источник uid (Google `sub` vs Firebase `localId`) и перекеит Firestore-путь → **не zero-behavior-change**. Оставлен в KEEP. Решение: оставить `signInWithIdp` только ради `localId`, либо мигрировать uid на Google `sub` (behavior-affecting, re-sync).
- **cloud-platform scope** в `google_auth_start_signin` (`:193`) — был нужен только для firestore_admin (теперь удалён), значит латентен. Сужение до `openid email profile` меняет consent-screen → re-consent для существующих юзеров. Отдельный follow-up.
- **Firebase Hosting tails** — в просканированных Rust/JS-путях Hosting-кода не найдено (нет `firebase.json`/hosting deploy). Не покрыто; нужен отдельный поиск по корню репо/CI.

---

## 4. Tier 2 — removal steps

**Behavior impact:** гость теряет ровно одну способность — read-only Firestore fallback при офлайн-хосте. Эта ветка уже инертна (tunnel_url не инжектится, origin не `*.web.app`), поэтому основной axum/Tailscale read/write путь не затронут. Tier-3 owner-sync и cloud-share PUSH (`load_config`, `CloudShareConfig`, `service_account_json`) не трогаются. CorsLayer остаётся (нужен Tailscale 100.x + localhost), убираются только два Firebase-Hosting предиката.

1. **delete-file** `share_assets/guest_firestore.js` целиком (единственный продюсер `window.HanniGuest.firestore`).
   *Gate:* `rg 'guest_firestore' desktop/src-tauri/src` — только route/include/script-tag (шаги 3-5).
2. **share_server.rs** `landing()` — свернуть кортеж `(ctx, firestore_json)` обратно в `ctx`. Убрать `:151-156` (`load_config(...).map(...).unwrap_or("null")`), `, firestore_json` (`:157`), `.replace("{{FIRESTORE}}", ...)` (`:165`), комментарий 148-150. **КРИТИЧНО:** не трогать `sync_share::load_config` (Tier-3, используется sync_owner + sync_share:397/430/690) — только этот call-site.
   *Gate:* `rg 'FIRESTORE|firestore_json|load_config' share_server.rs` пусто; `cargo check`.
3. **share_server.rs** — убрать route `:68` (`/s/{token}/assets/guest_firestore.js`) и `asset_js_firestore,` из `use crate::share_static::{...}` (26-31).
   *Gate:* `rg 'asset_js_firestore' share_server.rs` пусто.
4. **share_static.rs** — удалить `asset_js_firestore` (`:15-19` с `include_str!`). Иначе `include_str!` упадёт после шага 1.
   *Gate:* `rg 'asset_js_firestore|guest_firestore' share_static.rs` пусто; `cargo check`.
5. **guest.html** — убрать `:33` `<script ...guest_firestore.js>` и `firestore: {{FIRESTORE}},` из `window.__SHARE__` (`:19`).
   *Gate:* `rg 'firestore|FIRESTORE' guest.html` пусто.
6. **guest_recipes.js** — удалить `fs`/`haveTunnel` (`:48-53`), `fetchFromFirestore()` (`:55-66`). В `fetchListAndCatalog` (68-79) → прямой `return await api('/recipes');`. В `fetchRecipe` (81-97) → прямой `return await api('/recipes/'+id);`. Сохранить LINK_REVOKED/EXPIRED/FORBIDDEN handling (он из `api()`). Обновить комментарии 48-50, 215.
   *Gate:* `node --check`; `rg 'firestore|fs\.list|fs\.get|haveTunnel'` пусто.
7. **guest_products.js** (`:26-41`) — убрать `fs`/`haveTunnel`, `fetchCatalog()` → `return await api('/products');`.
   *Gate:* `node --check`; rg пусто.
8. **guest_meal_plan.js** (`:24-57`) — убрать fs/haveTunnel, `fetchPlan()` → `return await api('/meal-plan?date='+encodeURIComponent(dateIso));`.
   *Gate:* `node --check`; rg пусто.
9. **guest_memory.js** (`:26-40`) — убрать fs/haveTunnel, `fetchBlacklist()` → `return await api('/blacklist');`.
   *Gate:* `node --check`; rg пусто.
10. **guest_fridge.js** (`:7-37`) — убрать fs/haveTunnel; `loadCatalog()` → `(await api('/recipes')).catalog || []`; `loadFridgeItems()` → `(await api('/fridge')).items || []`.
    *Gate:* `node --check`; rg пусто.
11. **guest.js** — убрать tunnel const + comment (`:4-7`), `isStaticOrigin`/`HOST_OFFLINE` guard в `api()` (`:16-24`), Firestore cache-invalidate после write (`:39-45`). `base` становится просто `/s/${token}`. Подрезать renderShell-комментарии (126-134); сама `renderShell` и её вызов из guest.html:45 **остаются** (нужны axum-landing).
    *Gate:* `node --check`; `rg 'firestore|tunnel_url|HOST_OFFLINE|web\.app|firebaseapp'` пусто; `base`/`api` резолвятся в `/s/{token}`.
12. **share_server.rs** CORS (`:84-105`) — убрать предикаты `s == "https://hanni-2e5d0.web.app"` и `...firebaseapp.com`, переписать комментарий 84-88 под Tailscale/localhost. **KEEP** CorsLayer + localhost + `http://100.` (CGNAT) клаузы.
    *Gate:* `cargo check`; `rg 'web.app|firebaseapp' share_server.rs` пусто; 100./localhost живы.
13. **Cargo.toml:32** — `tower-http` с `cors` оставить, поправить комментарий «CORS for guests on Firebase Hosting» → Tailscale/localhost. `cors` feature **не убирать**.
14. **commands_share.rs** — поправить стейл-комментарии (`:109-112` в create_share_link). `:36` уже корректен, оставить. Переписать 109-112: writes доходят до axum через Tailscale Funnel/cloudflared.
    *Gate:* `rg 'Firestore|Firebase Hosting' commands_share.rs` пусто (или только корректный `:36`).
15. **sync_share.rs** — поправить два комментария (`:502-504` в build_snapshot, `:647-649` в scope_covers), убрав Firebase-Hosting-guest framing. **НЕ удалять** `load_config`/`CloudShareConfig`/`project_id`/`api_key`/snapshot-поле `tunnel_url` (Tier-3).
    *Gate:* `cargo check`; `rg 'Firebase Hosting' sync_share.rs` пусто.
16. **share-modal.js** — `:261` 429-warning сейчас обещает «читать через Firebase» при офлайн-хосте — переписать (read-fallback больше нет). `:246` комментарий «static cloud URL (Firebase Hosting)» → Tailscale Funnel. `:25` кнопка ☁ «Облачный share (Firebase)» открывает Tier-3 config-modal — оставить или переименовать (ask user, см. §9).
    *Gate:* `node --check`; `:261` не обещает Firebase read.
17. **projects.yaml** — убрать `share_assets/guest_firestore.js` (`:386`); обновить `share.description` (`:356`), убрав «Firebase Hosting fallback». Subprojects `guest_cloud` (393-411) и `firestore` (412-419) описывают **отдельный** Hosting-деплой (`apps/guest-cloud`, `firebase/`) — НЕ часть axum Tier-2; флагнуть юзеру (§9), не удалять в этом проходе.
    *Gate:* `rg 'guest_firestore' projects.yaml` пусто под share_assets; `/audit-projects` без phantom/orphan.
18. **verify** — `UPDATER_GITHUB_TOKEN=dummy cargo check` из `desktop/src-tauri` (baseline: 17 pre-existing warnings). `node --check` на каждый правленый JS. Smoke-test живого share-link (auto-reload + screenshot.sh + /auto/eval): `/s/{token}`, проверить рендер recipes/products/meal_plan/memory/fridge, reads/writes через axum, без console-ошибок про `HanniGuest.firestore`.

---

## 5. Tier 3 — decision matrix + открытый вопрос

Tier 3 — это **рабочий механизм sync**, его нельзя удалить, только заменить. Две опции:

- **Option A «Tailscale-only»** — выкинуть Firestore owner-sync, sync идёт только через `lan_sync.rs` по tailnet (CGNAT 100.x). Data-plane уже готов в lan_sync (курсоры, tombstones, LWW, 15s loop, key-authed сервер на :8244, CGNAT auto-upgrade).
- **Option B «cloud-broker»** — заменить Firestore тонким self-hosted KV-брокером на существующем axum share-хосте (1 новая SQLite-таблица `sync_broker` + 2 роута). Store-and-forward сохраняется.

| Dimension | Option A (Tailscale-only) | Option B (cloud-broker) |
|---|---|---|
| **Async/offline (store-and-forward)** | **ПОТЕРЯН.** Строго синхронный P2P: оба устройства должны быть одновременно онлайн в tailnet. Телефон офлайн неделю → ничего не накапливается удалённо, догон только когда оба онлайн вместе (`lan_sync.rs:235` «peer unreachable», loop `:328` двигается только при ответе peer'а). | **СОХРАНЁН.** Брокер — durable inbox: A пушит блобы пока B офлайн; B забирает всё после своего курсора, когда сам выйдет онлайн. A не обязан быть онлайн в момент pull. Та же модель, что Firestore. |
| **Remote (разные сети)** | Работает (CGNAT 100.x спанит сети), но только при одновременной онлайн-сессии обоих. Ограничение временно́е, не топологическое. | Работает через Funnel public URL или 100.x. Одновременная онлайн-сессия не нужна — каждому нужен только хост, достижимый в момент его sync. |
| **Infra/ops** | **Минимум.** Нет cloud, Firebase, cloudflared, квот, always-on box. Убирает Spark-quota + 429-backoff целиком. | Low-medium. Нет Firebase/JWT/квот, НО **один девайс обязан держать share-host + Funnel** (always-on требование, которого у Firestore не было). Single-disk durability. |
| **Implementation effort** | Medium. Data-plane готов; работа = удаление + аккуратная **релокация хелперов** + **новый provisioning UX** (peer discovery, durable persistence, key handoff, enable-by-default). | Medium. ~1 таблица + ~2 роута + /health (~120 строк `share_routes_sync.rs`) + ~30-строчный auth + точечный rewire sync_owner.rs (swap transport, keep apply/cursor/LWW). |
| **Lines of new code** | Net **negative** на Rust-стороне (в основном удаление). Новый код — provisioning UX (больше JS/UI). | Net **positive**: ~150+ host-side строк + client rewire (вводится relay, которого в A нет). |
| **Risk** | Medium: (1) compile-coupling — `lan_sync.rs:14` берёт `upsert_row/row_to_json/get_setting/set_setting` из sync_owner, удалять нельзя, только relocate; (2) accepted product regression — нет offline-догона. | Medium иной формы: нет функц. регрессии и нет relocation-ловушки, НО добавляет single-host failure mode (хост офлайн → весь cross-device sync корректно-но-молча стоит; митигировать `/sync/health` в UI) + single-disk durability. |
| **Зависимость от Tailscale на ОБОИХ** | **Hard на обоих.** Android Tailscale на паузе → sync молча стоит. Fallback-пути нет. | **Softer/asymmetric.** Каждому нужна достижимость только ХОСТА в момент его sync, не одновременная liveness peer'а. Через Funnel хост достижим даже вне tailnet. |

### Открытый вопрос (отвечает пользователь — единственный фактор выбора)

> **Нужна ли тебе асинхронная offline-доставка (store-and-forward)?** Должны ли изменения с одного устройства догонять другое, когда оба **НЕ онлайн одновременно** — например, телефон лежал выключенным / с паузнутым Tailscale неделю, потом включился и подтянул всё, при этом Mac в этот момент мог быть выключен?
>
> - **ДА (offline-догон обязателен)** → нужен брокер → **Option B**.
> - **НЕТ (устраивает, что sync идёт только когда оба онлайн в tailnet)** → **Option A**.
>
> Это единственный dimension, разводящий варианты; остальные следуют из него. (Per instructions — ни одна из опций не рекомендуется до твоего ответа.)

### Эскиз шагов выбранной опции (применять ТОЛЬКО после решения)

**Если Option A:**
1. **RELOCATE** перед удалением: вынести `upsert_row`, `upsert_event_category`, `row_to_json`, `table_columns`, `sqlite_to_json`, `json_to_sqlite`, `get_setting`, `set_setting`, `device_id` из `sync_owner.rs` в `lan_sync.rs` (или новый `sync_core.rs`); обновить `use crate::sync_owner::{...}` в `lan_sync.rs:14`.
2. **DELETE** Firestore data-plane в sync_owner.rs (`resolve_creds`, `encode_doc`/`decode_doc`/`decode_field`, `patch_doc`, `run_query`, `push_table`/`push_tombstones`/`pull_all`, `push_inner`/`pull_inner`) + команды `cloud_owner_push`/`pull`/`debug_owner_list`.
3. **DELETE** `sync_owner_auto.rs` + вызов `start_auto_sync_loop` (`lib.rs:923`) + `mod sync_owner_auto;` (`:58`).
4. **UNREGISTER** в `lib.rs` invoke_handler (845-852); решить судьбу `cloud_owner_set/get_uid` + Google-sign-in-for-sync (cross-cutting с share — confirm).
5. **REMOVE JS** Firestore-sync surface: `cloud-owner-sync.js`, `sync-trigger.js`-дебаунс, `cloud_owner_*` вызовы в `cloud-share-modal.js`. **ВАЖНО:** также убрать import `requestPush` из `state.js:3` (см. §9 silent-break).
6. **ADD provisioning** (реальная новая работа): peer discovery через `tailscale status --json`, durable peer persistence + re-probe на boot, one-step key+peer handoff (QR/deep-link).
7. **FLIP** `lan_sync_enabled=true` по умолчанию при onboarding (иначе удаление auto-loop молча оставит sync выключенным).
8. **projects.yaml** тем же таском.

**Если Option B:**
1. **AUTH**: per-owner `sync_key` (HMAC-SHA256(host_secret, owner_uid) или 192-bit random) в app_settings, копируется peer'ам при pairing. Каждый запрос: `Authorization: Bearer <sync_key>` + `X-Owner-Uid`. Reuse `rate_limit_check`.
2. **TABLE** в `db.rs`: `sync_broker(owner_uid, doc_id, seq, device_id, updated_at, table_name, deleted, body, PRIMARY KEY(owner_uid, doc_id))` + unique index `(owner_uid, seq)`. `doc_id` = `{table}_{id}` / `tombstone_{table}_{id}`; `seq` — dense per-owner monotonic.
3. **ENDPOINT push** `POST /sync/{uid}/push` (новый `share_routes_sync.rs`): auth + upsert каждого doc с новым seq в одной транзакции; returns `{max_seq}`. Заменяет `patch_doc`; client батчит dirty docs.
4. **ENDPOINT pull** `GET /sync/{uid}/pull?since={seq}&limit=500`: `SELECT ... WHERE owner_uid=? AND seq>? ORDER BY seq LIMIT ?`. Заменяет `run_query`.
5. **CLIENT rewire** в `sync_owner.rs`: pull-курсор из ISO `_synced_at` → integer `seq`; `resolve_creds` → `resolve_broker` (base URL = `share_funnel_url`/`share_tailscale_url`, uid, sync_key); `patch_doc`→`broker_push`, `run_query`→`broker_pull`; убрать `json_to_field`/`decode_field`. **KEEP** весь apply/cursor/tombstone/LWW путь байт-в-байт.
6. **HOST DISCOVERY**: surface `share_funnel_url`/`share_tailscale_url` + sync_key peer'у при pairing → `sync_broker_url` в app_settings.
7. **HEALTH**: `/sync/health` (зеркало `/share/health`) → `{ok, max_seq}` чтобы UI показал «sync paused — host offline». GC не нужен (UPSERT держит 1 строку на doc_id, таблица bounded by live-doc count).
8. **projects.yaml** тем же таском.

---

## 6. Recommended sequencing

1. **Tier 1 first (safe, zero-behavior).** §3 шаги 1-9. Полностью независим, не блокирует ничего, не требует решений. Делать в отдельной ветке (`try/firebase-off` уже подходит).
2. **Tier 2 (safe, теряет только офлайн read-fallback).** §4 шаги 1-18. Подтвердить с юзером, что отказ от офлайн-read приемлем (§9). Независим от Tier 1, но логически следует за ним.
3. **Tier 3 — ТОЛЬКО после ответа на открытый вопрос §5.** Не начинать никакой Tier-3 код, пока юзер не выбрал A или B. Внутри выбранной опции: для A — relocation хелперов строго ПЕРЕД удалением sync_owner; для B — таблица + роуты ПЕРЕД client-rewire.

> **Не смешивать тиры в одном коммите.** Tier 1 / Tier 2 / Tier 3 — отдельные коммиты (и желательно отдельные PR), чтобы откат был точечным.

---

## 7. Verification gates

**После Tier 1:**
- `UPDATER_GITHUB_TOKEN=dummy cargo check` из `desktop/src-tauri` — exit 0, без НОВЫХ ошибок (baseline ~17 warnings).
- `grep -rn get_firebase_id_token desktop/` и `grep -rn get_google_access_token desktop/` — пусто.
- `/audit-projects` — нет phantom `firestore_admin.rs`.
- Smoke: открыть cloud-share-modal, проверить что Google sign-in box рендерится (`google_auth_status` жив), owner-sync статус читается.

**После Tier 2:**
- `cargo check` exit 0, без новых ошибок.
- `node --check` на каждый правленый JS (`guest*.js`, `share-modal.js`).
- `rg 'firestore|HanniGuest.firestore|web\.app|firebaseapp'` по `share_assets/` — пусто.
- Live smoke через auto-reload + screenshot.sh + /auto/eval: `/s/{token}` рендерит recipes/products/meal_plan/memory/fridge; read + write (напр. добавить комментарий) через axum проходит; console без ошибок про `HanniGuest.firestore`.

**После Tier 3 (выбранная опция):**
- `UPDATER_GITHUB_TOKEN=dummy cargo check` — особое внимание на dangling `sync_owner::` ссылки в `lib.rs` и `lan_sync.rs` после relocation (Option A).
- `node --check` на каждый правленый JS.
- **Ручной Mac↔Android тест** по tailnet (CGNAT):
  - Option A: строку, созданную на Mac пока Android онлайн, видно на Android ≤15s; подтвердить документированное офлайн-поведение (Android офлайн → нет догона, пока оба не онлайн вместе).
  - Option B: два девайса конвергируют через push→pull; с хостом в спячке push очередится (курсор не двигается) и чисто возобновляется при возврате хоста; `/sync/health` показывает host-down.
- Подтвердить, что после удаления Firestore auto-loop sync реально включён (Option A: `lan_sync_enabled` true; иначе sync молча off).

---

## 8. Handoff to mechanical removal workflow (cargo-check fix-loop)

Этот документ — план; механическое применение идёт через cargo-check fix-loop:

1. **Применяй по одному шагу за раз**, в порядке §3 → §4 → (§5 после решения). После каждого структурного шага — соответствующий gate.
2. **Rust fix-loop:** после каждого delete-file / delete-function / deregister прогоняй `UPDATER_GITHUB_TOKEN=dummy cargo check`. Компилятор поймает: unresolved module (забыл `mod`), unused import (забыл `use`), unused variable (забыл локал), `generate_handler!` ссылку на удалённую команду, dangling `sync_owner::` после relocation. Чини по списку ошибок, не накатывай fix-on-fix.
3. **JS fix-loop:** после каждой правки JS — `node --check <file>` (белый экран от SyntaxError — известная ловушка). Особо: убери `{{FIRESTORE}}` placeholder и его `.replace` **синхронно** — иначе в guest.html останется невалидный `firestore: {{FIRESTORE}},`.
4. **Manifest contract:** каждое delete-file / new-file → правка `projects.yaml` **тем же коммитом**; финальный `/audit-projects` без orphan/phantom.
5. **Порядок внутри тира критичен** там, где это помечено (firestore_admin delete ПЕРЕД удалением `get_google_access_token`; helper relocation ПЕРЕД gut sync_owner; таблица+роуты ПЕРЕД client-rewire). Нарушение порядка = compile break.
6. **Стоп-точки для юзера:** signInWithIdp (Tier 1 §3.6), retire `apps/guest-cloud`/`firebase/` (Tier 2 §9), выбор A/B (Tier 3 §5). Не проходить их без явного ответа.

---

## 9. Outstanding gaps (от критиков)

### Coverage critic — пропущенные сайты (Firestore-комментарии, не в карте)
- `desktop/src/main.js:392` — комментарий `// → Firestore CRDT → Mac.` (карта вообще не ссылается на main.js). Обновить при Tier-3.
- `share_tunnel.rs:52` — doc-comment `/// no Firestore, no quota. Falls back to None...`. Файл отсутствует в карте; комментарий корректен, но упоминает Firestore.
- `lan_sync.rs:4` — комментарий `// rows straight over HTTP — no cloud, no Firestore, no quota.`. Корректен.

### Silent-break critic — скрытые поломки (КРИТИЧНО для Tier 3 и регистраций)
- **`state.js:3,17-23` + `sync-trigger.js:23`** — центральный `invoke()` wrapper в `state.js` (это `_shared.core`, **не** проект `sync`, removal-шаг по sync-файлам его НЕ заденет) импортит `requestPush` из `sync-trigger.js` и дёргает `cloud_owner_push` после КАЖДОГО write по всему приложению. Если `cloud_owner_push` удалён/unregistered, а import жив → либо мёртвый round-trip на каждую мутацию (swallowed `.catch`), либо, если удалить `sync-trigger.js` без правки `state.js:3` → ES-import fails → **белый экран всего фронтенда**. **Удалять оба синхронно.**
- **`lan_sync.rs:14`** — LAN sync (не-Firebase фича) импортит `get_setting/row_to_json/set_setting/upsert_row` из `sync_owner.rs`. Наивное удаление sync_owner ломает LAN-компиляцию. Хелперы **relocate, не delete** (центральный риск Option A).
- **`share_server.rs:143-156,165`** — guest landing читает `cloud_share_config` через `sync_share::load_config` для `{{FIRESTORE}}`. Tier-2 шаг 2 это убирает; не трогать сам `load_config` (Tier-3).
- **`guest.html:19,33`** — если убрать route asset, но оставить литерал → 404 на script + невалидный `firestore: {{FIRESTORE}},` → parse error гостевой страницы. Tier-2 шаг 5 синхронизирует.
- **`main.js:406`** (Android) — `invoke('bg_sync_enable', {intervalMinutes: 15})` планирует WorkManager pull, который драйвит `cloud_owner_push/pull`. Если Firestore chain удалён, а `bg_sync_enable` + WorkManager job живы → Android молча планирует задачу, пушащую в никуда (battery drain). Учесть при Tier-3.
- **`commands_meta.rs:1396-1434`** — HTTP API (always-spawned) регистрирует `/oauth/google/callback` → `google_auth::handle_oauth_callback`. Это **не** Firebase-файл, в Firebase-scoped removal не попадёт. Если `google_auth.rs` удалить/выпотрошить → compile break или orphaned 500-роут. Tier-1 KEEP'ит `handle_oauth_callback`, так что в рамках §3 безопасно.
- **`lib.rs:841-862`** — invoke_handler регистрирует Firestore-зависимые команды; JS-вызыватели, пережившие removal, бросят «command not found»: `cloud-owner-sync.js`, `cloud-share-modal.js` (`google_auth_*`, `cloud_owner_get_auto`), `share-modal.js` (`cloud_share_push`), `sync-trigger.js` (`cloud_owner_push`). При Tier-3 дерегистрировать команды и править/удалять JS-вызыватели в одном проходе. `lib.rs:923` безусловно spawn'ит `start_auto_sync_loop` — удаление `sync_owner_auto.rs` без правки этой строки = compile break.

### Silent-break critic — manifest/deps
- **`projects.yaml:434`** — листит `sync_share_auto.rs`, которого **нет на диске** (phantom). Manifest-driven removal попытается удалить несуществующий файл. Сигнал: `sync`-список манифеста ненадёжен как removal-источник. Почистить через `/audit-projects --fix-phantoms`.
- **`Cargo.toml:39` `jsonwebtoken`, `:40` `sha2`** — Firebase-only (service-account JWT + owner_uid derivation). При **Tier-3** (не раньше — Tier-1/2 их не трогают) после удаления Firestore-auth станут unused; убрать в Tier-3-проходе выбранной опции. (`tower-http`/`cors` и `percent-encoding` — KEEP, нужны Tailscale CORS / vacancy.rs.)
- **`sync_owner.rs:39-52` `resolve_creds`** — цепляет три config-read (`cloud_share_config`, `google_auth_session`, `google_auth_config`) через sync_share.rs И google_auth.rs. Tier-3 removal должен учесть все ключи app_settings (`cloud_share_config`, `google_auth_config`, `google_auth_session`, `google_auth_pending_state`), иначе auto-loop эррорит каждый тик.

### Требуют явного ответа юзера (стоп-точки)
- **Tier 1:** `signInWithIdp` как источник `uid` — оставить ради `localId` или мигрировать на Google `sub` (re-sync). cloud-platform scope narrowing (re-consent).
- **Tier 2:** retire ли отдельный Hosting-деплой `apps/guest-cloud/*` + `firebase/*` (firestore.rules/indexes, .firebaserc) + `scripts/firebase-setup.sh` — **separate pass**, не в Tier-2. ⚠️ `firestore.rules` управляет и owner-sync write-путями (Tier-3) — не удалять без аудита. Приемлем ли отказ от офлайн-read для прод-гостей. Rebrand ли ☁ «Облачный share (Firebase)» (`share-modal.js:25`) — UI-copy решение.
- **Tier 3:** открытый вопрос §5 (async offline → A или B). Судьба Google-sign-in-for-sync (cross-cutting с share-табом — не вырывать без проверки, что share работает).
