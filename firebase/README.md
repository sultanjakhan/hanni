# Hanni → Firebase share-link mirror

Эта директория — **Stage A** (read-only) для облачной share-link, чтобы гости видели данные и **когда Mac закрыт**. Полное чтение/запись — Stage B (Cloud Function для JWT) — будет позже.

## Что уже готово

- `firestore.rules` — security rules с проверкой `share_token` через custom JWT claim.
- `firestore.indexes.json` — composite indexes под все query гостя.
- `firebase.json` + `.firebaserc` — project config.
- `apps/guest-cloud/` — статический guest UI (Firebase Web SDK v10), деплоится на Firebase Hosting.
- `desktop/src-tauri/src/sync_share.rs` — Tauri команды для cloud-share + snapshot builder. Stage A: **dry-run** — собирает payload, но в Firestore пока не пишет (раскомментировать `firestore_upsert_snapshot` когда нужно).
- `scripts/firebase-setup.sh` — однокликовая настройка project + Firestore + Hosting.

## Что нужно сделать тебе (один раз, ~10 минут)

### 1. Залогиниться в Firebase

```bash
firebase login
```

Откроется системный браузер, жмёшь «Allow».

### 2. Запустить setup-скрипт

```bash
cd /Users/sultanbekjakhanov/hanni
./scripts/firebase-setup.sh
```

Скрипт создаёт project (Spark plan, бесплатно), включает Firestore + Hosting, деплоит rules/indexes/static-guest и печатает Web SDK config.

### 3. Скачать **service-account JSON** (для Hanni-side writes)

Без service-account Hanni не сможет писать в Firestore (rules блокируют unauth writes). Spark plan не позволяет Cloud Functions, поэтому единственный путь — service-account JSON.

1. Открой <https://console.firebase.google.com/project/hanni-share-XXXXX/settings/serviceaccounts/adminsdk>
2. «Generate new private key» → подтверди → скачается JSON файл
3. **Никогда** не коммить его — это полный admin доступ

### 4. Сохранить web-config + service-account в Hanni

DevTools console в открытом Hanni:

```js
await window.__TAURI__.core.invoke('cloud_share_set_config', {
  projectId: 'hanni-share-abc12345',
  apiKey:    'AIza...',
  serviceAccountJson: `<полное содержимое скачанного JSON одной строкой>`
});
```

Конфиг сохраняется в SQLite `app_settings`, owner_uid генерится автоматически и не меняется. Service-account JSON дальше backend никуда не уходит.

### 5. Сохранить web-config в guest-side

Замени `apps/guest-cloud/firebase-config.js`:

```js
window.HANNI_FIREBASE_CONFIG = {
  apiKey: "AIza...",
  authDomain: "hanni-share-abc12345.firebaseapp.com",
  projectId: "hanni-share-abc12345",
  appId: "1:..."
};
```

И задеплой:

```bash
firebase deploy --only hosting --project hanni-share-abc12345
```

### 6. Тестовый push

DevTools:

```js
// Dry-run сначала — посмотреть counts
await __TAURI__.core.invoke('cloud_share_push', { shareId: 1, dryRun: true });
// → {"status":"dry-run","counts":{"recipes":25,...}}

// Боевой push в Firestore
await __TAURI__.core.invoke('cloud_share_push', { shareId: 1 });
// → {"status":"ok","written":{"recipes":25,"share_links":1,...}}
```

Открой `https://hanni-share-abc12345.web.app/?t=<share_token>` — гость видит данные, **даже если Hanni закрыт**.

## Stage B (после Stage A)

Stage A — read-only для гостя. Гость видит данные но не пишет (write требует custom JWT с `share_token` claim, который генерится только Cloud Function — а CF недоступны на Spark plan).

Варианты для Stage B:
- **Перейти на Blaze plan** (pay-as-you-go, на нашем объёме = ~$0/мес). Развернуть Cloud Function `mintToken`. Гость зайдёт через `signInWithCustomToken`, rules пропустят писать.
- **Альтернативно** — запросы гостя проксируются через Hanni-side endpoint, когда Hanni online (текущий cloudflared туннель). Когда Hanni offline — write-операции дисэйблятся в UI.

Я могу реализовать оба варианта в следующей сессии. По умолчанию рекомендую первый — Cloud Function маленький, запросы редкие, Blaze не выставит счёт пока в free quota (1M invocations/мес).

## Команды-памятка

```bash
# Логин (один раз)
firebase login

# Настройка (один раз)
./scripts/firebase-setup.sh

# Передеплой rules после правок
firebase deploy --only firestore:rules

# Передеплой guest UI
firebase deploy --only hosting

# Логи
firebase functions:log

# Удалить project (если что-то пошло не так)
firebase projects:delete <project-id>
```

## Лимиты бесплатного Spark plan

- Firestore: 1 GiB / 50k reads / 20k writes / 20k deletes per day.
- Hosting: 10 GB transfer/month, 1 GB storage.
- Этого хватит для one-user share с горизонтом ~годы.

## URL гостя

После setup: `https://hanni-share-<hash>.web.app/?t=<share_token>`

Этот URL **не меняется** между запусками Hanni и работает когда Mac закрыт (для read-операций сразу, для write — после Stage B).
