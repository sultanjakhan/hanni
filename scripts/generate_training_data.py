#!/usr/bin/env python3
"""Generate high-quality agentic training data for Hanni LoRA fine-tuning.
Focus: maximizing action block success rate across all action types."""

import json, os

HANNI_DIR = os.path.expanduser("~/Library/Application Support/Hanni")
SYSTEM_PROMPT_FILE = os.path.join(HANNI_DIR, "system_prompt.txt")
OUTPUT_DIR = os.path.join(HANNI_DIR, "training")

# Read the actual system prompt
with open(SYSTEM_PROMPT_FILE) as f:
    SYSTEM_PROMPT = f.read().strip()

# Context template (simulates what the model sees at runtime)
CONTEXT = """[Current context]
Today: {today}, {weekday}
Time: {time}
Active App: {app}"""

def make_example(messages, today="2026-02-13", weekday="Thursday", time="14:30", app="Safari"):
    """Create a training example with system prompt + context + conversation."""
    ctx = CONTEXT.format(today=today, weekday=weekday, time=time, app=app)
    full_messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
    ]
    # First user message gets context prepended
    first_user = True
    for role, content in messages:
        if role == "user" and first_user:
            content = f"{ctx}\n\n{content}"
            first_user = False
        full_messages.append({"role": role, "content": content})
    return {"messages": full_messages}

# ═══════════════════════════════════════════════
# TRAINING EXAMPLES — Agentic Tasks
# ═══════════════════════════════════════════════

examples = []

# ── 1. MEMORY: remember facts ──
examples.append(make_example([
    ("user", "Меня зовут Султан, мне 21 год, я учусь в Назарбаев Университете"),
    ("assistant", "Приятно познакомиться, Султан! Запомнила тебя.\n```action\n{\"action\": \"remember\", \"category\": \"user\", \"key\": \"name\", \"value\": \"Султан, 21 год, учится в Назарбаев Университете\"}\n```"),
]))

examples.append(make_example([
    ("user", "Я люблю аниме и программирование на Rust"),
    ("assistant", "О, отличное сочетание! Запомню.\n```action\n{\"action\": \"remember\", \"category\": \"preferences\", \"key\": \"hobbies\", \"value\": \"Аниме и программирование на Rust\"}\n```"),
]))

examples.append(make_example([
    ("user", "Мой друг Артём работает в Google"),
    ("assistant", "Круто, запомнила про Артёма!\n```action\n{\"action\": \"remember\", \"category\": \"people\", \"key\": \"Артём\", \"value\": \"Друг, работает в Google\"}\n```"),
]))

examples.append(make_example([
    ("user", "Я обычно ложусь в 2 ночи и встаю в 9"),
    ("assistant", "Полуночник! Запомню твой режим.\n```action\n{\"action\": \"remember\", \"category\": \"habits\", \"key\": \"sleep_schedule\", \"value\": \"Ложится в 2 ночи, встаёт в 9\"}\n```"),
]))

examples.append(make_example([
    ("user", "Не люблю когда мне пишут на вы"),
    ("assistant", "Понял, буду только на ты!\n```action\n{\"action\": \"remember\", \"category\": \"preferences\", \"key\": \"communication\", \"value\": \"Не любит обращение на вы, только на ты\"}\n```"),
]))

# ── 2. MEMORY: recall/search ──
examples.append(make_example([
    ("user", "Что ты знаешь обо мне?"),
    ("assistant", "Давай посмотрю, что помню!\n```action\n{\"action\": \"recall\", \"category\": \"user\"}\n```"),
    ("user", "[Action result: name=Султан, 21 год | university=Назарбаев Университет]"),
    ("assistant", "Ты Султан, 21 год, учишься в Назарбаев Университете. Что-то хочешь дополнить?"),
]))

examples.append(make_example([
    ("user", "Кто такой Артём?"),
    ("assistant", "Сейчас поищу в памяти!\n```action\n{\"action\": \"search_memory\", \"query\": \"Артём\", \"limit\": 5}\n```"),
]))

# ── 3. CALENDAR: create events with date math ──
examples.append(make_example([
    ("user", "Завтра экзамен по математике в 10:00"),
    ("assistant", "Добавила экзамен на завтра! Удачи тебе.\n```action\n{\"action\": \"create_event\", \"title\": \"Экзамен по математике\", \"date\": \"2026-02-14\", \"time\": \"10:00\", \"duration\": 120, \"category\": \"education\"}\n```"),
], today="2026-02-13", weekday="Thursday"))

examples.append(make_example([
    ("user", "В понедельник созвон с командой в 15:00, примерно на полчаса"),
    ("assistant", "Записала созвон на понедельник!\n```action\n{\"action\": \"create_event\", \"title\": \"Созвон с командой\", \"date\": \"2026-02-16\", \"time\": \"15:00\", \"duration\": 30}\n```"),
], today="2026-02-13", weekday="Thursday"))

examples.append(make_example([
    ("user", "Через неделю дедлайн по курсовой"),
    ("assistant", "Поставила дедлайн! Не забудь заранее подготовиться.\n```action\n{\"action\": \"create_event\", \"title\": \"Дедлайн: курсовая\", \"date\": \"2026-02-20\", \"time\": \"\", \"duration\": 0, \"category\": \"education\"}\n```"),
], today="2026-02-13", weekday="Thursday"))

examples.append(make_example([
    ("user", "Поставь напоминание на послезавтра — купить подарок маме"),
    ("assistant", "Готово! Не забудешь.\n```action\n{\"action\": \"create_event\", \"title\": \"Купить подарок маме\", \"date\": \"2026-02-15\", \"time\": \"\", \"duration\": 0}\n```"),
], today="2026-02-13", weekday="Thursday"))

# ── 4. MONEY: transactions ──
examples.append(make_example([
    ("user", "Потратил 3500 на кофе и завтрак"),
    ("assistant", "Ого, кофе с завтраком за 3500! Записала.\n```action\n{\"action\": \"add_transaction\", \"transaction_type\": \"expense\", \"amount\": 3500, \"category\": \"food\", \"description\": \"Кофе и завтрак\"}\n```"),
]))

examples.append(make_example([
    ("user", "Получил зарплату 450000"),
    ("assistant", "Отлично, зарплата пришла! Записала доход.\n```action\n{\"action\": \"add_transaction\", \"transaction_type\": \"income\", \"amount\": 450000, \"category\": \"salary\", \"description\": \"Зарплата\"}\n```"),
]))

examples.append(make_example([
    ("user", "Заплатил за квартиру 120к"),
    ("assistant", "Записала оплату!\n```action\n{\"action\": \"add_transaction\", \"transaction_type\": \"expense\", \"amount\": 120000, \"category\": \"housing\", \"description\": \"Аренда квартиры\"}\n```"),
]))

examples.append(make_example([
    ("user", "Купил наушники за $150"),
    ("assistant", "Хороший выбор! Записала.\n```action\n{\"action\": \"add_transaction\", \"transaction_type\": \"expense\", \"amount\": 150, \"category\": \"electronics\", \"description\": \"Наушники\", \"currency\": \"USD\"}\n```"),
]))

# ── 5. FOOD: logging ──
examples.append(make_example([
    ("user", "Поел плов на обед, примерно 600 калорий"),
    ("assistant", "Вкусный выбор! Записала обед.\n```action\n{\"action\": \"log_food\", \"meal_type\": \"lunch\", \"name\": \"Плов\", \"calories\": 600}\n```"),
]))

examples.append(make_example([
    ("user", "На завтрак овсянка с бананом"),
    ("assistant", "Здоровый завтрак!\n```action\n{\"action\": \"log_food\", \"meal_type\": \"breakfast\", \"name\": \"Овсянка с бананом\", \"calories\": 350, \"protein\": 12, \"carbs\": 60, \"fat\": 8}\n```"),
]))

examples.append(make_example([
    ("user", "Съел бургер на ужин"),
    ("assistant", "Записала ужин!\n```action\n{\"action\": \"log_food\", \"meal_type\": \"dinner\", \"name\": \"Бургер\", \"calories\": 550}\n```"),
]))

# ── 6. HEALTH: logging ──
examples.append(make_example([
    ("user", "Сегодня спал 6 часов, выпил 8 стаканов воды, прошёл 12000 шагов"),
    ("assistant", "Мало спал, но зато активный день! Записала.\n```action\n{\"action\": \"log_health\", \"sleep\": 6, \"water\": 8, \"steps\": 12000}\n```"),
]))

examples.append(make_example([
    ("user", "Вешу 75 кг"),
    ("assistant", "Записала вес!\n```action\n{\"action\": \"log_health\", \"weight\": 75}\n```"),
]))

# ── 7. MOOD: logging ──
examples.append(make_example([
    ("user", "Настроение ужасное, поссорился с другом"),
    ("assistant", "Бывает... Надеюсь, помиритесь. Записала.\n```action\n{\"action\": \"log_mood\", \"mood\": 2, \"note\": \"Поссорился с другом\", \"trigger\": \"конфликт\"}\n```"),
]))

examples.append(make_example([
    ("user", "Сегодня отличный день! Сдал экзамен на отлично"),
    ("assistant", "Поздравляю! Заслуженно. Записала отличное настроение!\n```action\n{\"action\": \"log_mood\", \"mood\": 5, \"note\": \"Сдал экзамен на отлично\", \"trigger\": \"успех\"}\n```"),
]))

# ── 8. JOURNAL ──
examples.append(make_example([
    ("user", "Хочу записать дневник. Настроение 4, энергия 3, стресс 2. Благодарен за хорошую погоду. Сегодня победа — закончил проект."),
    ("assistant", "Отличная рефлексия! Записала в дневник.\n```action\n{\"action\": \"save_journal\", \"mood\": 4, \"energy\": 3, \"stress\": 2, \"gratitude\": \"Хорошая погода\", \"wins\": \"Закончил проект\"}\n```"),
], time="22:30"))

# ── 9. MEDIA: tracking ──
examples.append(make_example([
    ("user", "Начал смотреть аниме Jujutsu Kaisen, пока на 5 серии из 24"),
    ("assistant", "О, крутой выбор! Записала в коллекцию.\n```action\n{\"action\": \"add_media\", \"media_type\": \"anime\", \"title\": \"Jujutsu Kaisen\", \"status\": \"watching\", \"progress\": 5, \"total_episodes\": 24}\n```"),
]))

examples.append(make_example([
    ("user", "Дочитал книгу Atomic Habits, ставлю 9 из 10"),
    ("assistant", "Отличная книга! Рада, что понравилась.\n```action\n{\"action\": \"add_media\", \"media_type\": \"book\", \"title\": \"Atomic Habits\", \"status\": \"completed\", \"rating\": 9}\n```"),
]))

examples.append(make_example([
    ("user", "Посмотрел фильм Интерстеллар — шедевр, 10/10"),
    ("assistant", "Согласна, шедевр! Нолан гений.\n```action\n{\"action\": \"add_media\", \"media_type\": \"movie\", \"title\": \"Интерстеллар\", \"status\": \"completed\", \"rating\": 10}\n```"),
]))

examples.append(make_example([
    ("user", "Добавь игру Elden Ring, хочу поиграть"),
    ("assistant", "Добавила в список! Отличная игра.\n```action\n{\"action\": \"add_media\", \"media_type\": \"game\", \"title\": \"Elden Ring\", \"status\": \"planned\"}\n```"),
]))

# ── 10. FOCUS MODE ──
examples.append(make_example([
    ("user", "Мне надо сосредоточиться, заблокируй все соцсети на 2 часа"),
    ("assistant", "Заблокировала! Продуктивной работы.\n```action\n{\"action\": \"start_focus\", \"duration\": 120, \"sites\": [\"youtube.com\", \"twitter.com\", \"reddit.com\", \"tiktok.com\", \"instagram.com\", \"vk.com\"]}\n```"),
]))

examples.append(make_example([
    ("user", "Сними блокировку"),
    ("assistant", "Сняла! Отдыхай.\n```action\n{\"action\": \"stop_focus\"}\n```"),
]))

# ── 11. ACTIVITY TRACKING ──
examples.append(make_example([
    ("user", "Начинаю учиться, запусти таймер"),
    ("assistant", "Таймер запущен! Продуктивной учёбы.\n```action\n{\"action\": \"start_activity\", \"name\": \"Учёба\", \"category\": \"education\"}\n```"),
]))

examples.append(make_example([
    ("user", "Всё, закончил работу"),
    ("assistant", "Молодец! Остановила таймер.\n```action\n{\"action\": \"stop_activity\"}\n```"),
    ("user", "[Action result: Stopped activity 'Работа над Hanni' after 2h 15m]"),
    ("assistant", "2 часа 15 минут — хорошая сессия! Отдохни немного."),
]))

# ── 12. FITNESS ──
examples.append(make_example([
    ("user", "Потренировался час в зале, делал жим и присед"),
    ("assistant", "Сильная тренировка! Записала.\n```action\n{\"action\": \"add_workout\", \"type\": \"strength\", \"title\": \"Жим лёжа + присед\", \"duration\": 60}\n```"),
]))

examples.append(make_example([
    ("user", "Пробежал 5 км за 25 минут"),
    ("assistant", "Отличный темп! 5 мин/км — хороший результат.\n```action\n{\"action\": \"add_workout\", \"type\": \"running\", \"title\": \"Бег 5 км\", \"duration\": 25}\n```"),
]))

# ── 13. NOTES ──
examples.append(make_example([
    ("user", "Запиши заметку: идеи для хакатона — AI-ассистент для врачей, рекомендации по питанию на основе анализов"),
    ("assistant", "Записала! Интересные идеи, особенно первая.\n```action\n{\"action\": \"add_note\", \"title\": \"Идеи для хакатона\", \"content\": \"1. AI-ассистент для врачей\\n2. Рекомендации по питанию на основе анализов\"}\n```"),
]))

# ── 14. HOME ITEMS ──
examples.append(make_example([
    ("user", "Купил молоко, хлеб и яйца"),
    ("assistant", "Записала продукты!\n```action\n{\"action\": \"add_product\", \"name\": \"Молоко\", \"category\": \"dairy\", \"location\": \"fridge\"}\n```"),
    ("user", "[Action result: Product added: Молоко]"),
    ("assistant", "Теперь хлеб.\n```action\n{\"action\": \"add_product\", \"name\": \"Хлеб\", \"category\": \"bakery\"}\n```"),
    ("user", "[Action result: Product added: Хлеб]"),
    ("assistant", "И яйца.\n```action\n{\"action\": \"add_product\", \"name\": \"Яйца\", \"category\": \"dairy\", \"location\": \"fridge\"}\n```"),
    ("user", "[Action result: Product added: Яйца]"),
    ("assistant", "Всё записала! Молоко, хлеб и яйца в списке."),
]))

# ── 15. GOALS ──
examples.append(make_example([
    ("user", "Хочу создать цель — прочитать 12 книг за год"),
    ("assistant", "Отличная цель! По книге в месяц — реально.\n```action\n{\"action\": \"create_goal\", \"tab\": \"Hobbies\", \"title\": \"Прочитать 12 книг\", \"target\": 12, \"unit\": \"книг\", \"deadline\": \"2026-12-31\"}\n```"),
]))

examples.append(make_example([
    ("user", "Прочитал ещё одну книгу, обнови цель"),
    ("assistant", "Обновляю прогресс!\n```action\n{\"action\": \"update_goal\", \"id\": 1, \"current\": 3}\n```"),
]))

# ── 16. SYSTEM COMMANDS ──
examples.append(make_example([
    ("user", "Открой гитхаб"),
    ("assistant", "Открываю!\n```action\n{\"action\": \"open_url\", \"url\": \"https://github.com\"}\n```"),
]))

examples.append(make_example([
    ("user", "Убавь звук до 30"),
    ("assistant", "Готово!\n```action\n{\"action\": \"set_volume\", \"level\": 30}\n```"),
]))

examples.append(make_example([
    ("user", "Что у меня в буфере обмена?"),
    ("assistant", "Сейчас посмотрю!\n```action\n{\"action\": \"get_clipboard\"}\n```"),
    ("user", "[Action result: https://arxiv.org/abs/2401.12345]"),
    ("assistant", "У тебя ссылка на arxiv-статью: https://arxiv.org/abs/2401.12345. Что-то интересное читаешь?"),
]))

examples.append(make_example([
    ("user", "Сколько свободного места на диске?"),
    ("assistant", "Сейчас проверю!\n```action\n{\"action\": \"run_shell\", \"command\": \"df -h / | tail -1 | awk '{print $4}'\"}\n```"),
    ("user", "[Action result: 142Gi]"),
    ("assistant", "142 ГБ свободно. Места достаточно!"),
]))

# ── 17. TASKS ──
examples.append(make_example([
    ("user", "Создай задачу: доделать презентацию к пятнице, высокий приоритет"),
    ("assistant", "Создала задачу!\n```action\n{\"action\": \"create_task\", \"title\": \"Доделать презентацию\", \"priority\": \"high\", \"due_date\": \"2026-02-14\"}\n```"),
], today="2026-02-12", weekday="Wednesday"))

# ── 18. PROACTIVE MEMORY — model remembers facts without explicit ask ──
examples.append(make_example([
    ("user", "Завтра у меня собеседование в Яндекс, волнуюсь немного"),
    ("assistant", "Понимаю волнение, но ты справишься! Буду держать кулаки. Давай запомню и поставлю в календарь.\n```action\n{\"action\": \"remember\", \"category\": \"tasks\", \"key\": \"interview\", \"value\": \"Собеседование в Яндекс, 2026-02-14\"}\n```"),
    ("user", "[Action result: Remembered]"),
    ("assistant", "И добавлю в календарь!\n```action\n{\"action\": \"create_event\", \"title\": \"Собеседование в Яндекс\", \"date\": \"2026-02-14\", \"time\": \"\", \"duration\": 0, \"category\": \"career\"}\n```"),
    ("user", "[Action result: Event created]"),
    ("assistant", "Готово! Запомнила и поставила в календарь. Хочешь, помогу подготовиться?"),
]))

examples.append(make_example([
    ("user", "Я переехал в Алматы из Астаны"),
    ("assistant", "О, серьёзный переезд! Запомню.\n```action\n{\"action\": \"remember\", \"category\": \"user\", \"key\": \"city\", \"value\": \"Живёт в Алматы, переехал из Астаны\"}\n```"),
]))

# ── 19. COMPLEX MULTI-STEP TASKS ──
examples.append(make_example([
    ("user", "Запланируй мне день: утром тренировка в 8, в 10 учёба, в 14 обед с Артёмом, вечером кино"),
    ("assistant", "Планирую твой день! Сначала тренировка.\n```action\n{\"action\": \"create_event\", \"title\": \"Тренировка\", \"date\": \"2026-02-14\", \"time\": \"08:00\", \"duration\": 60, \"category\": \"fitness\"}\n```"),
    ("user", "[Action result: Event created]"),
    ("assistant", "Теперь учёба.\n```action\n{\"action\": \"create_event\", \"title\": \"Учёба\", \"date\": \"2026-02-14\", \"time\": \"10:00\", \"duration\": 180, \"category\": \"education\"}\n```"),
    ("user", "[Action result: Event created]"),
    ("assistant", "Обед с Артёмом!\n```action\n{\"action\": \"create_event\", \"title\": \"Обед с Артёмом\", \"date\": \"2026-02-14\", \"time\": \"14:00\", \"duration\": 60}\n```"),
    ("user", "[Action result: Event created]"),
    ("assistant", "И кино на вечер.\n```action\n{\"action\": \"create_event\", \"title\": \"Кино\", \"date\": \"2026-02-14\", \"time\": \"19:00\", \"duration\": 120, \"category\": \"entertainment\"}\n```"),
    ("user", "[Action result: Event created]"),
    ("assistant", "Готово! Твой день на завтра:\n- 8:00 — Тренировка\n- 10:00 — Учёба\n- 14:00 — Обед с Артёмом\n- 19:00 — Кино\n\nНасыщенный день!"),
], today="2026-02-13"))

# ── 20. AMBIGUOUS REQUESTS handled correctly ──
examples.append(make_example([
    ("user", "Запиши 2000 за такси"),
    ("assistant", "Записала расход на такси!\n```action\n{\"action\": \"add_transaction\", \"transaction_type\": \"expense\", \"amount\": 2000, \"category\": \"transport\", \"description\": \"Такси\"}\n```"),
]))

examples.append(make_example([
    ("user", "Добавь воду — 3 литра"),
    ("assistant", "Записала!\n```action\n{\"action\": \"log_health\", \"water\": 3}\n```"),
]))

examples.append(make_example([
    ("user", "Начал Solo Leveling"),
    ("assistant", "О, классная манхва! Добавлю в коллекцию.\n```action\n{\"action\": \"add_media\", \"media_type\": \"manga\", \"title\": \"Solo Leveling\", \"status\": \"watching\"}\n```"),
]))

# ── 21. CASUAL CHAT (no action needed) ──
examples.append(make_example([
    ("user", "Привет, как дела?"),
    ("assistant", "Привет, Султан! У меня всё отлично, жду твоих задач. Как у тебя день?"),
]))

examples.append(make_example([
    ("user", "Что думаешь про искусственный интеллект?"),
    ("assistant", "Ну, я сама — искусственный интеллект, так что немного предвзята! Если серьёзно, AI сейчас на интересном этапе: локальные модели становятся достаточно умными для повседневных задач, а вот до AGI ещё далеко. Тебя какой аспект интересует — технический или философский?"),
]))

# ── 22. NOTIFICATION ──
examples.append(make_example([
    ("user", "Напомни мне через 30 минут выпить воду"),
    ("assistant", "Напомню!\n```action\n{\"action\": \"send_notification\", \"title\": \"Hanni\", \"body\": \"Султан, время пить воду!\"}\n```"),
]))

# ── 23. PRODUCTS with expiry ──
examples.append(make_example([
    ("user", "Купил курицу, срок до 16 февраля, положил в холодильник"),
    ("assistant", "Записала! Напомню, если будет истекать.\n```action\n{\"action\": \"add_product\", \"name\": \"Курица\", \"category\": \"meat\", \"expiry_date\": \"2026-02-16\", \"location\": \"fridge\"}\n```"),
]))

# ── 24. HOME SUPPLIES ──
examples.append(make_example([
    ("user", "Добавь в хозяйство: туалетная бумага, 6 рулонов"),
    ("assistant", "Добавила!\n```action\n{\"action\": \"add_home_item\", \"name\": \"Туалетная бумага\", \"quantity\": 6, \"unit\": \"рулонов\", \"category\": \"hygiene\"}\n```"),
]))

# ── 25. CALENDAR SYNC ──
examples.append(make_example([
    ("user", "Синхронизируй календарь на март"),
    ("assistant", "Синхронизирую!\n```action\n{\"action\": \"sync_calendar\", \"month\": 3, \"year\": 2026}\n```"),
]))

# ── 26. USING MEMORY CONTEXT ──
examples.append(make_example([
    ("user", "Как мне подготовиться к собеседованию?"),
    ("assistant", "Судя по моим заметкам, у тебя собеседование в Яндекс! Вот план подготовки:\n\n1. **Алгоритмы** — порешай задачи на LeetCode (medium)\n2. **System Design** — почитай про масштабирование\n3. **Проекты** — подготовь рассказ про Hanni как pet-project\n4. **Soft skills** — подумай над ответами на \"расскажи о себе\"\n\nХочешь, поставлю напоминания на подготовку?"),
]))

# ── 27. SHELL + action result handling ──
examples.append(make_example([
    ("user", "Покажи мой IP адрес"),
    ("assistant", "Сейчас проверю!\n```action\n{\"action\": \"run_shell\", \"command\": \"curl -s ifconfig.me\"}\n```"),
    ("user", "[Action result: 185.22.174.55]"),
    ("assistant", "Твой внешний IP: 185.22.174.55"),
]))

examples.append(make_example([
    ("user", "Сколько оперативки используется?"),
    ("assistant", "Проверяю!\n```action\n{\"action\": \"run_shell\", \"command\": \"memory_pressure | head -1\"}\n```"),
    ("user", "[Action result: The system has 36 GB of RAM installed, 22 GB used]"),
    ("assistant", "Из 36 ГБ используется 22 ГБ. 14 ГБ свободно — запас есть!"),
]))

# ── 28. macOS INFO ──
examples.append(make_example([
    ("user", "Что сейчас играет?"),
    ("assistant", "Сейчас посмотрю!\n```action\n{\"action\": \"get_music\"}\n```"),
    ("user", "[Action result: Now playing: Kendrick Lamar - HUMBLE. in Apple Music]"),
    ("assistant", "Играет Kendrick Lamar — HUMBLE. Хороший вкус!"),
]))

examples.append(make_example([
    ("user", "Что у меня в календаре сегодня?"),
    ("assistant", "Проверяю!\n```action\n{\"action\": \"get_calendar\"}\n```"),
    ("user", "[Action result: Today: 14:00 Созвон с командой (30 min), 19:00 Кино (2h)]"),
    ("assistant", "Сегодня у тебя:\n- 14:00 — Созвон с командой (30 мин)\n- 19:00 — Кино (2 часа)\n\nДо созвона ещё есть время!"),
]))

# ── 29. DELETE EVENT ──
examples.append(make_example([
    ("user", "Удали созвон из календаря"),
    ("assistant", "Сначала найду его!\n```action\n{\"action\": \"get_calendar\"}\n```"),
    ("user", "[Action result: Events: id=42 Созвон с командой 14:00, id=43 Кино 19:00]"),
    ("assistant", "Нашла, удаляю созвон.\n```action\n{\"action\": \"delete_event\", \"id\": 42}\n```"),
    ("user", "[Action result: Event deleted]"),
    ("assistant", "Готово! Созвон удалён. Остался только кино в 19:00."),
]))

# ── 30. FORGET memory ──
examples.append(make_example([
    ("user", "Забудь мой старый никнейм"),
    ("assistant", "Удаляю!\n```action\n{\"action\": \"forget\", \"category\": \"user\", \"key\": \"nickname\"}\n```"),
]))

# ═══════════════════════════════════════════════
# Write training files
# ═══════════════════════════════════════════════

os.makedirs(OUTPUT_DIR, exist_ok=True)

# Split: 80% train, 10% valid, 10% test
total = len(examples)
train_end = max(int(total * 0.8), 1)
valid_end = train_end + max(int(total * 0.1), 1)

splits = {
    "train.jsonl": examples[:train_end],
    "valid.jsonl": examples[train_end:valid_end],
    "test.jsonl": examples[valid_end:],
}

for filename, data in splits.items():
    path = os.path.join(OUTPUT_DIR, filename)
    with open(path, "w") as f:
        for ex in data:
            f.write(json.dumps(ex, ensure_ascii=False) + "\n")
    print(f"{filename}: {len(data)} examples")

print(f"\nTotal: {total} examples written to {OUTPUT_DIR}")
print("Ready for: python3 -m mlx_lm lora --model mlx-community/Qwen3-32B-4bit --data '{}'".format(OUTPUT_DIR))
