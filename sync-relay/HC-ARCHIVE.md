# HC public-value archive v1

Кандидат содержит Android encoder/registry для всех 41 конкретных Record в `androidx.health.connect:connect-client:1.1.0`. Он подключён к foreground-импорту, отдельному WorkManager worker, feature-gated read permissions, общей SQLite-схеме и зашифрованному relay v2. Проверки ниже используют синтетические данные; установка на телефоны и реальная доставка ещё не подтверждены.

Импорт хранит курсоры отдельно по типам и коммитит страницу, исходные ревизии и очередь отправки вместе. Первое чтение истории идёт от новых записей к старым; перед сканированием сохраняется Changes token, после сканирования воспроизводятся исправления и удаления. Ограниченный проход чередует типы, чтобы длинная история одного типа не задерживала остальные. Истёкший token помечает пробел в обнаружении удалений; отсутствие записи в ограниченном сканировании не считается удалением.

В relay одна свежая ещё не зашифрованная raw-ревизия получает место перед историей через один пакет. Между такими пакетами продвигается старая очередь. Существующие зашифрованные пакеты не переписываются и сохраняют порядок.

Raw SleepSessionRecord преобразуется в локальные представления сна, календаря и таймлайна отдельно от доставки архива. Один подтверждённый `sleep_source_store_id` является источником представления сна на всех устройствах; архивы остальных источников сохраняются. Исправление меняет существующие принадлежащие проекции строки, подтверждённое удаление удаляет только их. Ручные и исторические непривязанные строки не усваиваются по совпадению времени. Локальные представления исключены из облачного и старых транспортов; каждый получатель строит их из raw самостоятельно.

## Файлы и интеграция

- `scripts/check-health-apk-permissions.py` из корня репозитория сверяет registry с фактическим merged manifest готового APK через официальный `aapt2`. Запуск: `python scripts/check-health-apk-permissions.py --apk <candidate.apk> --aapt2 <android-sdk/build-tools/version/aapt2>`. Для текущего кандидата проверено 41 типа, 38 отдельных read permissions плюс history/background; пропусков нет. Такая проверка нашла и закрыла отсутствие READ_VO2_MAX; наличие разрешения не доказывает его выдачу устройством или наличие данных у провайдера.
- `src/RawHealthRecordCodec.kt` → `desktop/src-tauri/android-plugin/src/main/java/com/sultanjakhan/hanni/RawHealthRecordCodec.kt`.
- `test/RawHealthRecordFixtures.kt`, `test/RawHealthRecordCodecTest.kt` → одноимённые файлы в `desktop/src-tauri/android-plugin/src/test/java/com/sultanjakhan/hanni/`.
- `codec-api-coverage.json`: проверенный getter inventory и соответствие явным адаптерам; 41 record, 91 адаптер с учётом вложенных структур/units/полиморфных dispatchers.
- `generate_codec.py`, `generate_fixtures.py`: генерация только во время разработки из закреплённого SDK; в APK не входят. Runtime reflection отсутствует. Reflection применяется только тестом, который независимо перечисляет getters реальных синтетических SDK объектов и проверяет полноту результата.

Production dependencies не добавляются: используются существующие HC 1.1.0, org.json и стандартные Kotlin/Java типы. Тесты используют существующие JUnit/Robolectric. Для Mindfulness в закреплённой версии необходим `@file:OptIn(androidx.health.connect.client.feature.ExperimentalMindfulnessSessionApi::class)`; наличие этого класса не заменяет runtime feature/grant check.

## Внешний контракт

`RawHealthRecordCodec.encode(record: Record): JSONObject` возвращает:

```json
{
  "v": 1,
  "sdk": "androidx.health.connect:connect-client:1.1.0",
  "record_type": "StepsRecord",
  "record": {
    "metadata": {},
    "startTime": {"seconds": "1700000000", "nanos": 123456789},
    "startZoneOffset": null,
    "endTime": {"seconds": "1700000600", "nanos": 123456806},
    "endZoneOffset": 19800,
    "count": "123"
  }
}
```

`metadata` в примере сокращено; настоящий encoder всегда записывает все семь полей Metadata: `recordingMethod`, `id`, `dataOrigin.packageName`, `lastModifiedTime`, nullable `clientRecordId`, `clientRecordVersion`, nullable `device`. Device содержит type/manufacturer/model, включая null.

`record_type` — фиксированное имя из registry, не runtime class-name reflection. Каждый public getter pinned SDK представлен одноимённым JSON полем в camelCase. Вычисляемые totals, агрегирование источников, локальное HH:mm и фильтрация стадий здесь отсутствуют. Массивы сохраняются полностью и в порядке, возвращённом SDK.

### Числа и время

| Тип public SDK | Формат v1 |
|---|---|
| String | JSON string с обычным JSON escaping |
| Nullable | Явный JSON `null`; ключ не удаляется |
| Boolean | JSON boolean |
| Int, включая enum codes | JSON integer; неизвестный enum code не заменяется fallback-строкой |
| Long | Десятичная строка со знаком при необходимости, без потери точности JavaScript Number |
| Double | Только объект `{"f64":"3ff0000000000000"}` — 16 lowercase hex digits исходных IEEE-754 binary64 bits |
| Instant | `{"seconds":"…","nanos":0..999999999}`, epoch seconds UTC |
| Duration | `{"seconds":"…","nanos":0..999999999}`, нормализованные signed seconds + positive nanos Java Duration |
| ZoneOffset | Числовой totalSeconds либо `null`; системный timezone не подставляется |

`f64` фиксирован для archive v1: это **не десятичная строка**, не integer value и не endian-dependent byte array. Он соответствует `Double.doubleToRawLongBits`, старший hex digit первым. `readF64(JSONObject)` строго проверяет один ключ и 16 lowercase hex digits; обратное преобразование — `Double.longBitsToDouble`. Сохраняются negative zero, subnormal values, infinity и исходные NaN payload bits. Любая будущая проекция должна отдельно проверять пригодность значения для вычислений/отображения; encoder ничего не заменяет нулём. Метод не является декодером Record или validator всего archive payload.

### Единицы измерения

Каждый unit object имеет `type`, `primary_unit` и все числовые представления, доступные через public getters pinned SDK. Все значения представлены `f64`. Например Length содержит meters/kilometers/miles/inches/feet; `primary_unit` равен `meters`.

| Unit type | Основной getter / JSON key |
|---|---|
| BloodGlucose | inMillimolesPerLiter / millimolesPerLiter |
| Energy | inKilocalories / kilocalories |
| Length | inMeters / meters |
| Mass | inGrams / grams |
| Percentage | value / value |
| Power | inWatts / watts |
| Pressure | inMillimetersOfMercury / millimetersOfMercury |
| Temperature | inCelsius / celsius |
| TemperatureDelta | inCelsius / celsius |
| Velocity | inMetersPerSecond / metersPerSecond |
| Volume | inLiters / liters |

Дополнительные unit representations намеренно сохраняются: SDK скрывает исходную единицу конструктора, а повторная конверсия одного double может отличаться округлением от другого public getter. Поэтому lossless здесь означает точное сохранение **публично наблюдаемых значений SDK**, а не внутренних protobuf Samsung, private SDK fields или первоначальной единицы до платформенной нормализации. Правило primary_unit — выбранная конвенция нашего формата; это не утверждение о внутреннем формате Health Connect.

Polymorphic planned goals/targets и ExerciseRouteResult имеют явный `type`, например `ExerciseCompletionGoal.DistanceGoal` или `ExerciseRouteResult.ConsentRequired`. Сохраняются все 9 completion goal variants, 8 performance target variants и Data/ConsentRequired/NoData. Недоступный маршрут не превращается в пустой список.

## Registry и границы ответственности

`descriptors` содержит name, KClass, readPermission string, requiredFeature для каждого из 41 типов. Это только описание: код не проверяет и не запрашивает разрешения и не обращается к Health Connect. Неизвестный runtime Record или неподдержанный nested subtype приводит к безопасному error code `hc_record_type_unsupported` / `hc_nested_type_unsupported`; частичный payload не возвращается.

Encoder не выполняет writeback в Health Connect, не читает Changes API, не выбирает source identity и не сохраняет токены. Это следующие слои с собственными транзакционными контрактами. Payload содержит медицинские/личные сведения: его нельзя логировать или включать в exception messages. Кодек не добавляет логи.

JSON object key order не объявляется отдельным canonical-JSON стандартом. Для chunk/hash transport необходимо фиксировать точные UTF-8 bytes созданного документа; получатель проверяет эти bytes до повторной сериализации. Большие массивы не обрезаются: тест содержит payload больше 60 000 bytes. До согласованного chunk transport такие payload нельзя подключать к нынешней очереди одиночных relay envelopes.

## Проверка

Синтетические тесты: registry и permissions всех 41 типов; сериализация/parse JSON и рекурсивное сравнение каждого public getter реального SDK, включая metadata и nested planned structures; signed zero/subnormals/NaN bits; malformed f64; nullable Nutrition; крупный HR массив без потери samples; safe error для неизвестного Record. Отдельные runtime branches маршрутов/полученные от провайдера server-assigned metadata должны дополнительно проверяться доступными SDK fixtures или instrumentation; construction-only тест не является proof реального Health Connect чтения.

Проверено: actual Kotlin main/test compile и 28/28 JUnit PASS (7 новых codec tests и 21 существующий), все 41 synthetic constructor исполнились. SHA трёх исходников до/после проверки совпали. Доказательства: `validation.json`, `compile-test.log`; проверку выполнил отдельный build-agent. `raw-health-codec.patch` содержит один новый production source и два test sources. Этот пакет не является APK, установкой, проверкой фонового Android запуска или end-to-end подтверждением доставки между устройствами.

## Проверяемый источник

AAR SHA-256: `ce1601af0ef671ec6a76b0f29d2af22acc578e408c7107d08ea23067d3822593`. Генератор останавливается при другом SDK artifact hash. Использованы [официальный API PlannedExerciseSessionRecord](https://developer.android.com/reference/kotlin/androidx/health/connect/client/records/PlannedExerciseSessionRecord) и [публичные единицы Length](https://developer.android.com/reference/kotlin/androidx/health/connect/client/units/Length); список getters в сгенерированном исходнике основан именно на локальном pinned AAR, а не на более новом онлайн-каталоге.
