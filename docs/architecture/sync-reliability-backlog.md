# Sync Reliability Backlog

Активный backlog по восстановлению автоматической синхронизации Hanni между
Android и Mac. Работа выполняется по одной задаче; следующая задача начинается
только после прохождения verification gate предыдущей.

## Подтверждённое исходное состояние

- Mac видит сон только по 2026-07-13 включительно.
- Health Connect permissions и Android WorkManager исправны.
- Mac (`192.168.100.121`) и телефон (`192.168.100.149`) доступны друг другу по Wi-Fi.
- LAN-конфигурация Mac указывает на неактивный Tailscale peer, а прямой запрос
  к телефону отклоняется из-за несовпадающего или отсутствующего shared key.
- GitHub owner-sync читает все primary keys как `INTEGER`, поэтому молча
  пропускает UUID/TEXT-таблицы, включая health и sleep.
- У share-links есть активные ссылки и накопленные dirty-флаги, но consumer
  очереди автоматического mirror-sync сейчас не запускается.

## Порядок выполнения

### Epic SR-1 — Корректность GitHub owner-sync

Цель: облачный store-and-forward синхронизирует все таблицы независимо от типа
SQLite primary key и не сообщает ложный успех.

- [x] **SR-1.1 — Поддержать INTEGER и TEXT primary keys**
  - Читать `id` как `rusqlite::types::Value`, как уже делает LAN-sync.
  - Убрать молчаливый `filter_map` для ошибок чтения ID.
  - Покрыть минимум `events` (INTEGER), `sleep_sessions` (TEXT), `health_log`
    (TEXT), `schedules` (TEXT).
  - Gate: Rust tests + `cargo check`; TEXT-ID строки входят в outgoing batch.
  - Выполнено 2026-07-16: покрыты `events`, `sleep_sessions`, `health_log`,
    `schedules`; ошибки decode больше не отбрасываются молча.

- [x] **SR-1.2a — Подготовить безопасный replay пропущенных таблиц**
  - Сбросить только cursors затронутых TEXT-ID таблиц, не весь sync state.
  - Gate: selective-reset и повторный запуск покрыты Rust tests.
  - Выполнено 2026-07-16: INTEGER, pull и LAN cursors сохраняются; на Mac все
    шесть TEXT-ID cursors уже отсутствуют, поэтому replay стартует с EPOCH.

- [ ] **SR-1.2b — Выполнить native rollout и live replay**
  - Доставить Rust-фикс на Mac и Android через release workflow.
  - Выполнить push/pull на обоих устройствах после резервной копии Mac DB.
  - Gate: последние строки сна совпадают, повторный sync идемпотентен и
    возвращает 0 новых изменений.

### Epic SR-2 — Надёжный LAN pairing и transport fallback

Цель: устройства соединяются без ручного копирования секретов и продолжают
работать при включении/выключении Tailscale.

- [ ] **SR-2.1 — Единое pairing-состояние**
  - Добавить безопасную передачу peer address + shared key между устройствами
    (QR/deep-link либо одноразовый pairing code).
  - Не синхронизировать LAN-secret через обычные sync-таблицы или логи.
  - Gate: чистая конфигурация телефона и Mac создаётся за один pairing flow;
    `/lan/sync` отвечает успешно с обеих сторон.

- [ ] **SR-2.2 — Wi-Fi/Tailscale candidate fallback**
  - Хранить несколько peer candidates, а не навсегда заменять Wi-Fi адрес
    Tailscale hint-ом.
  - Пробовать последний успешный адрес первым и переключаться при timeout.
  - Gate: sync проходит при Tailscale ON, затем без перенастройки при OFF.

- [ ] **SR-2.3 — Явный статус транспорта**
  - Показывать last success, выбранный transport и точную ошибку
    (`unreachable`, `bad key`, `disabled`) вместо общего зелёного статуса.
  - Gate: каждый из трёх отказов воспроизводится и корректно отражается в UI.

### Epic SR-3 — Сон: Health Connect → Hanni → Calendar → Mac

Цель: сон появляется автоматически в raw health views, Timeline и Calendar,
даже когда Android WebView не открыт.

- [ ] **SR-3.1 — Завершить background health pipeline**
  - После импорта worker должен запускать фактическую доставку, а не только
    записывать локальную Android DB.
  - Устранить расхождение между комментарием `HanniHealthWorker` и кодом.
  - Gate: закрытая Hanni импортирует новую sleep session и отправляет её.

- [ ] **SR-3.2 — Фоновый fan-out в Calendar и Timeline**
  - Создавать/обновлять производные события сна без зависимости от foreground JS.
  - Сохранить идемпотентность при повторных Health Connect reads и LAN/cloud pull.
  - Gate: одна sleep session создаёт ровно одно событие и один timeline block.

- [ ] **SR-3.3 — End-to-end проверка свежего сна**
  - Проверить Android raw DB → Mac raw DB → Sleep UI → Calendar → Timeline.
  - Gate: свежий сон появляется на Mac без ручного открытия Android Hanni.

### Epic SR-4 — Автоматическое обновление share-links

Цель: активные гостевые ссылки автоматически получают изменения, а dirty queue
не зависает. Реализация должна соответствовать актуальному firebase-off плану,
а не просто возвращать старую зависимость вслепую.

- [ ] **SR-4.1 — Выбрать живой mirror transport**
  - Сверить share-link offline requirements с
    `docs/architecture/firebase-off-plan.md`.
  - Зафиксировать: direct host-only или store-and-forward broker.
  - Gate: выбран один поддерживаемый data path без двух конкурирующих SSOT.

- [ ] **SR-4.2 — Восстановить consumer dirty queue**
  - Запускать mirror worker при старте Hanni.
  - Сохранять retry/backoff и не очищать flags при partial failure.
  - Gate: изменение recipe/product автоматически очищает соответствующий dirty
    flag и становится доступно через активную share-link.

- [ ] **SR-4.3 — Диагностика и live smoke-test**
  - Показывать last mirror success/error и размер очереди.
  - Проверить чтение и запись через реальную гостевую ссылку.
  - Gate: перезапуск Hanni догоняет накопленные изменения без ручного Push.

## Release gate

После завершения всех эпиков:

- Mac↔Android sync проверен при Wi-Fi only и Tailscale only.
- Новая sleep session проходит end-to-end при закрытом Android UI.
- GitHub replay не создаёт дубликатов и синхронизирует TEXT-ID таблицы.
- Share-link mirror автоматически догоняет изменения после перезапуска.
- `cargo check`, релевантные Rust/JS tests и production build проходят.
- Прод обновляется только после резервной копии Mac DB и проверки версии Android.
