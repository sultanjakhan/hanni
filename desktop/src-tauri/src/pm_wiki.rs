// ── pm_wiki.rs — PM learning project: wiki seed content ──
// Data module: the actual article text lives in the bundled Markdown files
// (pm_wiki_overview.md, pm_wiki_pages.md) so it stays easy to edit.

/// Legacy English skill name → Russian wiki page name. Order matches the
/// original `pm_skills()` seed so renaming existing installs is deterministic.
const RENAMES: &[(&str, &str)] = &[
    ("Discovery & User Research", "Discovery и исследования"),
    ("Prioritization", "Приоритизация"),
    ("Metrics & Analytics", "Метрики и аналитика"),
    ("Roadmapping", "Роадмап"),
    ("Stakeholder Management", "Управление стейкхолдерами"),
    ("User Stories & Requirements", "Требования и user stories"),
    ("A/B Testing & Experimentation", "A/B-тесты и эксперименты"),
    ("Go-to-Market", "Go-to-Market"),
    ("Technical Understanding", "Техническая грамотность"),
    ("Communication & Presentation", "Коммуникация и презентации"),
    ("Competitive Analysis", "Конкурентный анализ"),
    ("Strategy & Vision", "Стратегия и видение"),
    ("SQL & Data Analysis", "SQL и анализ данных"),
    ("Agile & Scrum", "Agile и Scrum"),
    ("UX/UI Fundamentals", "Основы UX/UI"),
    ("Customer Development", "Customer Development"),
    ("Pricing & Monetization", "Монетизация и ценообразование"),
    ("Growth & Retention", "Рост и удержание"),
];

/// Main wiki article for the PM project (Markdown with [[wiki-links]]).
pub fn overview() -> &'static str {
    include_str!("pm_wiki_overview.md")
}

/// Topic pages as `(legacy_en_name, ru_name, theory_markdown)`.
/// Theory is parsed from pm_wiki_pages.md, split on `<!--PAGE:Имя-->` markers.
pub fn skill_pages() -> Vec<(&'static str, &'static str, &'static str)> {
    let raw: &'static str = include_str!("pm_wiki_pages.md");
    let mut out = Vec::new();
    for chunk in raw.split("<!--PAGE:") {
        let chunk = chunk.trim_start();
        let Some(end) = chunk.find("-->") else { continue };
        let ru = chunk[..end].trim();
        let theory = chunk[end + 3..].trim();
        if let Some((en, _)) = RENAMES.iter().find(|(_, r)| *r == ru) {
            out.push((*en, ru, theory));
        }
    }
    out
}

/// Practice exercises as `(skill_ru_name, case_title, case_description)`.
pub fn seed_cases() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("Discovery и исследования", "Провести 5 проблемных интервью",
         "Выбери продукт, которым пользуешься. Сформулируй гипотезу о проблеме и проведи 5 интервью по The Mom Test — без наводящих вопросов. Зафиксируй факты о реальном поведении, а не мнения."),
        ("Discovery и исследования", "Построить JTBD и CJM",
         "Для знакомого продукта опиши 3 Job Stories в формате «Когда… я хочу… чтобы…» и нарисуй Customer Journey Map с шагами, болями и эмоциями пользователя."),
        ("Приоритизация", "Приоритизировать бэклог по RICE",
         "Возьми 8 продуктовых идей. Оцени каждую по Reach, Impact, Confidence, Effort и посчитай RICE-score. Объясни, почему топ-3 победили, и что осталось «за бортом»."),
        ("Приоритизация", "Разложить фичи по модели Kano",
         "Список из 6 фич распредели на Basic / Performance / Excitement. Объясни, какие нельзя не сделать, а какие дадут «вау-эффект»."),
        ("Метрики и аналитика", "Выбрать North Star Metric",
         "Для выбранного продукта предложи North Star Metric и обоснуй, почему она отражает ценность для пользователя, а не тщеславную (vanity) метрику. Разложи её на воронку AARRR."),
        ("Метрики и аналитика", "Посчитать юнит-экономику",
         "Даны CAC = 1200₽, средний чек 800₽/мес, churn 8%/мес. Посчитай LTV, LTV/CAC и payback period. Здоров ли бизнес? Что улучшать в первую очередь?"),
        ("Роадмап", "Собрать роадмап Now/Next/Later",
         "Составь outcome-based роадмап на 3 горизонта. Для каждого пункта укажи бизнес-цель и метрику, а не только фичу. Подготовь, что НЕ делаем и почему."),
        ("Управление стейкхолдерами", "Аргументированно сказать «нет»",
         "CEO просит срочную фичу вне роадмапа. Напиши ответ: что подвинется, какой trade-off, какие данные за приоритет. Цель — не отказ, а согласование (alignment)."),
        ("Требования и user stories", "Написать PRD на фичу",
         "Выбери фичу и напиши мини-PRD: проблема, аудитория, user stories, acceptance criteria (Given/When/Then), метрики успеха, edge cases."),
        ("A/B-тесты и эксперименты", "Спроектировать A/B-тест",
         "Гипотеза: новая формулировка CTA повысит конверсию регистрации. Опиши метрику, MDE, размер выборки, длительность, критерий принятия решения (p < 0.05) и риски."),
        ("A/B-тесты и эксперименты", "Разобрать ошибки эксперимента",
         "Тест показал «значимый» рост через 1 день. Найди проблемы: peeking, маленькая выборка, ошибка Type I, сезонность. Как провести корректно?"),
        ("Go-to-Market", "Составить GTM-план запуска",
         "Для новой фичи опиши positioning, messaging, каналы, сегмент запуска и launch-чеклист (документация, support, метрики, rollback plan)."),
        ("Техническая грамотность", "Описать работу через API",
         "Объясни на пальцах, что происходит при нажатии «Войти»: клиент, запрос к API, эндпоинт, ответ, база данных. Где фронтенд, где бэкенд?"),
        ("Коммуникация и презентации", "Подготовить elevator pitch",
         "Опиши продукт за 30 секунд: проблема → решение → почему вы. Затем сожми его до executive summary на 5 строк."),
        ("Конкурентный анализ", "Собрать feature-матрицу",
         "Выбери 3 конкурентов и продукт. Построй feature matrix, сделай SWOT по своему продукту и сформулируй уникальное позиционирование."),
        ("Стратегия и видение", "Сформулировать vision и OKR",
         "Напиши product vision на 3 года и один квартальный OKR: Objective + 3 измеримых Key Results. Проверь, что KR — это результаты, а не задачи."),
        ("SQL и анализ данных", "Написать запрос для когорт",
         "Дана таблица events(user_id, event, ts). Напиши SQL, считающий retention по неделям регистрации (когортный анализ) с GROUP BY и оконными функциями."),
        ("Agile и Scrum", "Провести sprint planning",
         "Свёрстай спринт на 2 недели: выбери задачи из бэклога по приоритету и capacity команды, опиши sprint goal и определи Definition of Done."),
        ("Основы UX/UI", "Нарисовать user flow и wireframe",
         "Выбери сценарий (например, онбординг). Нарисуй user flow по шагам и низкодетальный wireframe ключевого экрана. Где можно срезать шаги?"),
        ("Customer Development", "Проверить Product-Market Fit",
         "Опиши, как измеришь PMF для продукта: Sean Ellis Test (>40%), retention-кривая на плато, доля органического роста. Что делать, если PMF нет?"),
        ("Монетизация и ценообразование", "Выбрать модель монетизации",
         "Для продукта сравни freemium, подписку и transaction fee. Какую выбрать и почему? Предложи value-based цену и обоснуй её."),
        ("Рост и удержание", "Найти aha-moment и growth loop",
         "Определи activation-событие («aha-moment») продукта и спроектируй один growth loop (виральный, контентный или платный). Какую метрику он двигает?"),
    ]
}
