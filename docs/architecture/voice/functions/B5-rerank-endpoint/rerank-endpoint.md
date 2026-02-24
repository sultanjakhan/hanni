# B5 — /rerank Endpoint (voice_server.py)

> Часть B. FlashRank Reranking — серверная сторона

## Обзор

| Метрика | Значение |
|---------|----------|
| Endpoint | POST /rerank |
| Файл | desktop/voice_server.py:490 |
| Модель | ms-marco-MultiBERT-L-12 (ONNX, ~150MB) |
| Библиотека | flashrank |
| Lazy-load | ensure_reranker() с thread lock |
| Fallback | HTTP 501 если flashrank не установлен |

## Endpoint — POST /rerank

### Request

```json
POST http://127.0.0.1:8237/rerank
Content-Type: application/json

{
    "query": "кофе",
    "passages": [
        {"id": 1, "text": "[user] coffee=likes espresso"},
        {"id": 2, "text": "[user] age=21"},
        {"id": 3, "text": "[preferences] drink=чай"}
    ],
    "top_k": 30
}
```

| Поле | Тип | Обязательное | Описание |
|------|-----|-------------|---------|
| query | string | да | Запрос для ранжирования |
| passages | array | да | Массив объектов с id и text |
| passages[].id | int | нет | ID факта (default: index) |
| passages[].text | string | да | Текст для ранжирования |
| top_k | int | нет | Макс. результатов (default: 30) |

### Response (200)

```json
{
    "results": [
        {"id": 1, "text": "[user] coffee=likes espresso", "score": 0.87},
        {"id": 3, "text": "[preferences] drink=чай", "score": 0.34},
        {"id": 2, "text": "[user] age=21", "score": 0.02}
    ]
}
```

Отсортирован по score DESC. Возвращает максимум `top_k` элементов.

### Ошибки

| Код | Условие |
|-----|---------|
| 400 | query или passages пусты |
| 413 | Body > 64KB |
| 500 | Ошибка flashrank |
| 501 | flashrank не установлен |

## ensure_reranker() — voice_server.py:558

```python
_reranker = None
_reranker_lock = threading.Lock()

def ensure_reranker():
    global _reranker
    if _reranker is None:
        with _reranker_lock:
            if _reranker is None:
                try:
                    from flashrank import Ranker
                    _reranker = Ranker(model_name="ms-marco-MultiBERT-L-12")
                except ImportError:
                    logger.warning("flashrank not installed — reranking disabled")
                    return None
    return _reranker
```

- Double-checked locking (thread-safe)
- Первый вызов: ~2-5с загрузка модели ONNX
- При ImportError → None → HTTP 501 (не краш)
- Модель скачивается автоматически при первом запуске

## Зависимость

```bash
pip install flashrank
```

- Размер пакета: ~2MB
- Размер модели: ~150MB (скачивается при первом вызове Ranker)
- Zero torch — работает на чистом ONNX
- Мультиязычный (включая русский)

## Тестирование

```bash
curl -X POST http://127.0.0.1:8237/rerank \
  -H "Content-Type: application/json" \
  -d '{"query":"кофе","passages":[{"id":1,"text":"[user] coffee=likes espresso"},{"id":2,"text":"[user] age=21"}],"top_k":1}'
```

Ожидаемый результат: `{"results":[{"id":1,"text":"[user] coffee=likes espresso","score":...}]}`
