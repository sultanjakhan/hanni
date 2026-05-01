# Hanni → Firebase share-link mirror

Эта директория — **Stage A** (read-only) для облачной share-link, чтобы гости видели данные и **когда Mac закрыт**. Полное чтение/запись — Stage B (Cloud Function для JWT) — будет позже.

## Что уже готово

- `firestore.rules` — security rules с проверкой `share_token` через custom JWT claim.
- `firestore.indexes.json` — composite indexes под все query гостя.
- `firebase.json` + `.firebaserc` — project config.
- `apps/guest-cloud/` — статический guest UI (Firebase Web SDK v10), деплоится на Firebase Hosting.
- `desktop/src-tauri/src/sync_share.rs` — Tauri команды для cloud-share + snapshot builder. Stage A: **dry-run** — собирает payload, но в Firestore пока не пишет (раскомментировать `firestore_upsert_snapshot` когда нужно).
- `scripts/firebase-setup.sh` — однокликовая настройка project + Firestore + Hosting.

## Что нужно сделать тебе завтра (один раз, ~5 минут)

### 1. Залогиниться в Firebase

```bash
firebase login
```

Откроется системный браузер, жмёшь «Allow». Терминал ответит `Success!`.

### 2. Запустить setup-скрипт

```bash
cd /Users/sultanbekjakhanov/hanni
./scripts/firebase-setup.sh
```

Скрипт:
1. Создаёт project `hanni-share-<hash>` (Spark plan, бесплатно).
2. Включает Firestore native в `us-central1`.
3. Деплоит security rules + indexes.
4. Деплоит `apps/guest-cloud/` на Firebase Hosting.
5. В конце печатает Web SDK config — JSON примерно такого вида:

   ```js
   {
     "projectId": "hanni-share-abc12345",
     "appId": "1:...",
     "apiKey": "AIza...",
     "authDomain": "hanni-share-abc12345.firebaseapp.com",
     ...
   }
   ```

### 3. Сохранить config в `apps/guest-cloud/firebase-config.js`

Замени содержимое:

```js
window.HANNI_FIREBASE_CONFIG = {
  apiKey: "AIza...",
  authDomain: "hanni-share-abc12345.firebaseapp.com",
  projectId: "hanni-share-abc12345",
  appId: "1:..."
};
```

И задеплой ещё раз — `firebase deploy --only hosting --project hanni-share-abc12345`.

### 4. Сохранить config в Hanni (через DevTools, пока нет UI)

Открой Hanni, в DevTools console:

```js
await window.__TAURI__.core.invoke('cloud_share_set_config', {
  projectId: 'hanni-share-abc12345',
  apiKey:    'AIza...'
});
```

Это сохраняет конфиг в SQLite `app_settings` и генерит owner_uid. Один раз.

### 5. Проверить snapshot для существующей share-link

```js
const status = await window.__TAURI__.core.invoke('cloud_share_push', { shareId: 1 });
console.log(status);
// → {"status":"dry-run","counts":{"recipes":25,"recipe_ingredients":150,...}}
```

Это **dry-run** — подсчитает что было бы выгружено, но в Firestore пока не пишет.

## Что осталось доделать после твоей настройки

Я (Claude) включу боевую запись в Firestore — это правка одной строки в `sync_share.rs::cloud_share_push` (раскомментировать `firestore_upsert_snapshot`) + написать `firestore_upsert_snapshot` (Stage A) — REST PATCH на каждый документ через `https://firestore.googleapis.com/v1/projects/<id>/databases/(default)/documents/...?key=<api_key>` с auth по custom JWT.

После того как Stage A работает (read-only гость), Stage B — Cloud Function `mintToken` который принимает `share_token` и возвращает Firebase Custom JWT с claim `share_token`. Тогда guest может писать с проверкой rules.

Оценка: после твоего setup — ещё **1-2 сессии моей работы** до полностью рабочего сценария «гость видит и пишет, когда Mac закрыт».

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
