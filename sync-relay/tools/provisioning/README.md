# Локальная первоначальная выдача конфигураций

**Инструмент кандидата; реальное подключение ещё не выполнено.** Реальные конфигурации, файлы установленной Hanni, clipboard и Cloudflare не трогались. В package нет секретов. Helper только для Windows PowerShell 5.1/STA; выдаваемые конфигурации совместимы с Windows, S21, S20 и Mac.

## До реального запуска

Нужны подтверждённый Workers Free, фактический endpoint уже созданного dedicated Worker, source UUID выбранного S21, проверенные candidate install/build identity и существующая production `hanni.db`. Версия из исходников не доказывает идентичность установленного приложения. Флаги ниже — передача уже выполненной проверки, а не автоматическая проверка тарифа или подписи.

`types::hanni_data_dir()` использует `dirs::data_dir().join("Hanni")`. На Windows helper сверяет явно заданный путь с Windows KnownFolder ApplicationData + `Hanni`, проверяет существующий DB-файл и отклоняет reparse points. Он не определяет путь из рабочего каталога/имени checkout и не открывает строки DB.

Запускать в доверенной локальной PowerShell с фактическими значениями переменных; UUID/endpoint не выводить в публичную диагностику. Этот пример — шаблон, не готовые реальные значения:

```powershell
$setup = @{
  HanniDataDir = $verifiedProductionDataDir
  Endpoint = $verifiedRelayOrigin
  PrimarySourceStoreId = $verifiedS21Source
  FreePlanConfirmed = $true
  CandidateIdentityVerified = $true
}
& .\Hanni-RelayProvisioning.ps1 -Action Prepare @setup
& .\Hanni-RelayProvisioning.ps1 -Action Check @setup
```

Prepare создаёт общий CSPRNG ключ и key_id, четыре разные пары device_id/token в RAM, затем сохраняет **только DPAPI ciphertext** в новой private подпапке `relay-pairing/bootstrap.dpapi` внутри установленной Hanni. ACL — текущий Windows SID и SYSTEM, без наследования. Проверяет encrypt/decrypt roundtrip перед WriteThrough + атомарным rename без перезаписи. При существующем bundle, pairing directory или native `cloud-relay.credentials` отказывается создавать новое pairing. Не использовать повторную генерацию как ремонт связи.

Bundle защищён DPAPI CurrentUser. Это локальная копия для повторной выдачи, **не переносимая межплатформенная резервная копия**: потеря Windows профиля/его DPAPI ключей может сделать bundle недоступным. Полноценное восстановление после потери профиля этим package не решается. Managed строки нельзя гарантированно стереть из RAM; их не записывают на диск открытым текстом.

## Сервер получает только SHA256 bearer token

После создания Worker в рамках уже согласованного подключения и проверки Free:

```powershell
& .\Hanni-RelayProvisioning.ps1 -Action PublishHashes @setup `
  -NodeExe $verifiedNodeExe -WranglerEntry $verifiedWranglerCliEntry `
  -WranglerConfig $canonicalRelayWranglerConfig
```

Node entry — существующий pinned Wrangler `4.129.0`, `wrangler/wrangler-dist/cli.js`. Helper сначала вызывает read-only `versions list --json`, требует существующую версию, затем передаёт JSON `device_id -> SHA256(ASCII canonical bearer token)` через stdin в `versions secret put HANNI_DEVICE_TOKEN_HASHES`. Не передаёт token или E2E key; никакие приватные значения не попадают в argv. Raw stdout/stderr не выводятся. У дочернего Wrangler отключены disk logs/metrics, включено sanitization, CI/noninteractive. Новый OAuth/login не запускается; при недоступном существующем доступе операция должна завершиться ошибкой.

**Создаётся непубликованная версия.** Активация этой конкретной версии остаётся отдельным root действием после просмотра результата; helper не выполняет `deploy`/`versions deploy`, создание Worker, upgrade тарифа или регистрацию ресурсов. Метка новой версии — `hanni-pairing-v1`. Пока версия не активирована, hash upload нельзя считать рабочей серверной авторизацией. Повтор PublishHashes использует те же credentials из bundle, а не ротирует их.

Это намеренно `versions secret put`: у обычного `secret put` Wrangler 4.129.0 при отсутствующем Worker есть `createDraftWorker` с default/fallback **true** в CI. Read-before-write сам по себе не закрывает удаление Worker между проверкой и записью. Версионная команда вызывает PATCH существующей latest version и при исчезнувшем Worker отказывает, без draft fallback. Проверено по установленному официальному CLI source. [Документация команд](https://developers.cloudflare.com/workers/wrangler/commands/workers/#versions-secret-put).

## Выдача одному устройству

```powershell
& .\Hanni-RelayProvisioning.ps1 -Action Copy -Device windows @setup
# Затем выбрать соответственно s21, s20 или mac для их собственного кода.
```

Copy — единственная операция, выводящая plaintext из helper, и только в локальный clipboard по явному действию. Один JSON вставляется в новое Settings поле соответствующего устройства; native setter сохраняет его через существующий DPAPI/AndroidKeyStore/Mac Keychain. Helper напрямую не записывает native cfg и не обходит его identity checks. Выбор другого устройства не обновляет текущую Windows конфигурацию автоматически.

WinRT `SetContentWithOptions` устанавливает `IsAllowedInHistory=false` и `IsRoamable=false`. На старом/неподдержанном API helper отказывает; fallback в обычный Set-Clipboard отсутствует. [Microsoft ClipboardContentOptions](https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.datatransfer.clipboardcontentoptions?view=winrt-26100). Копия остаётся в текущем clipboard до замены; после вставки заменить clipboard обычным несекретным текстом. Эти флаги относятся к Windows history/cloud clipboard и не гарантируют поведение сторонних clipboard managers/remote-control программ. Не пересылать setup JSON через чат, shell arguments, лог, обычный файл или облачный QR сервис. Без доверенного способа локальной вставки на выбранное устройство этот шаг нельзя назвать выполненным.

## Проверки и границы

- `provisioning-core.test.ps1`: **6/6 PASS**, детерминированные синтетические values, фактические Windows DPAPI protect/unprotect/tamper rejection, ACL, encrypted file no-overwrite/roundtrip. Создаёт только synthetic ciphertext в personal/tmp. В sandbox DPAPI Protect отказал; узкий повтор был разрешён auto-review и прошёл.
- `publish-hashes.test.mjs`: **5/5 PASS**, mock process contract плюс настоящий child Node для отказа на malformed input; actual Wrangler ни разу не запускался. Проверены preflight перед write, hashes-only stdin, отказ missing Worker, delete race без draft fallback и fixed stdout без input/error content.
- Clipboard mutation, реальная выдача конфигураций, cloud upload/activation, native protected setter, Mac Keychain и installed APK identity не проверялись в этом пакете.
- Не является автоматическим переносом secrets на телефоны/Mac. Делает последующие первоначальные действия конкретными и повторяемыми; безопасная локальная вставка выполняется один раз на каждом устройстве.

Не запускать тестовый RNG с реальными устройствами и не использовать synthetic fixture bundle как production конфигурацию. `New-RelayRandom` используется только реальным Prepare; тесты передают отдельный детерминированный generator.
