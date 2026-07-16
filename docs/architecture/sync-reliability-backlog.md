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

- [x] **SR-1.2b — Выполнить native rollout и Mac live replay**
  - Доставить Rust-фикс на Mac и Android через release workflow.
  - Выполнить replay на Mac после резервной копии DB.
  - Gate: все затронутые TEXT-ID cursors дошли до последних строк, следующий
    replay не находит необработанных строк.
  - Выполнено 2026-07-16: v1.1.5 установлена на Mac и Android; исходный replay
    backlog шести TEXT-ID таблиц на Mac был полностью выгружен.

- [ ] **SR-1.2c — Настроить Android owner-sync и проверить GitHub replay**
  - Android v1.1.5 показывает owner-sync как не настроенный, auto-sync выключен.
  - Добавить безопасный перенос GitHub owner config либо явный pairing flow.
  - Gate: Android выполняет GitHub push, Mac pull получает свежий сон, второй
    push не повторяет уже выгруженные TEXT-ID строки.

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
  - Live finding 2026-07-16: Calendar дедуплицировал новые sleep events, но
    Timeline оставил дубли за 2026-07-12 и 2026-07-13 после первого LAN pull.

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

## Live rollout 2026-07-16

- Mac и Android обновлены до v1.1.5 без очистки данных; Mac DB backup прошёл
  `PRAGMA quick_check = ok`.
- Android Health Connect worker завершился успешно: `sleep=34`, `exercise=38`,
  `steps=31`, `hr=54847`.
- На Android LAN был выключен и не настроен, а ключ отличался от Mac. Оба
  устройства переведены на Wi-Fi peers с одним ключом; auto-LAN включён.
- Mac автоматически получил сон: `sleep_sessions` выросли с 90 до 93, последняя
  сессия — ночь 2026-07-15 → 2026-07-16. Повторные LAN циклы count не изменили.
- При Android UI в фоне оба WorkManager job завершились успешно; нативный LAN
  worker сообщил `sent=1009 received=37 deletes=3`. Большой heart-rate backfill
  продолжает идти пакетами, Mac cloud auto-sync включён как GitHub bridge.
- Android GitHub owner-sync всё ещё требует отдельного onboarding: текущий UI
  сообщает «не настроено», поэтому live Android→GitHub replay не подтверждён.
