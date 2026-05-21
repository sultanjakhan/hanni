// ── pm_matrix.rs — PM learning project: competency matrix seed content ──
// Structure (areas/competencies/skills + self-assessment scores) lives in
// pm_matrix_data.md; competency theory in pm_matrix_theory.md; the wiki
// intro in pm_wiki_overview.md. Kept in Markdown so it stays easy to edit.

pub struct MatrixSkill {
    pub name: String,
    pub score: i32,
    pub priority: bool,
}

pub struct MatrixCompetency {
    pub name: String,
    pub theory: String,
    pub skills: Vec<MatrixSkill>,
}

pub struct MatrixArea {
    pub name: String,
    pub competencies: Vec<MatrixCompetency>,
}

/// Main wiki article shown on the project's "Вики" pane.
pub fn overview() -> &'static str {
    include_str!("pm_wiki_overview.md")
}

/// Competency theory blocks, parsed from `<!--PAGE:Имя-->` markers.
fn parse_theory() -> Vec<(String, String)> {
    let raw = include_str!("pm_matrix_theory.md");
    let mut out = Vec::new();
    for chunk in raw.split("<!--PAGE:") {
        let chunk = chunk.trim_start();
        if let Some(end) = chunk.find("-->") {
            out.push((chunk[..end].trim().to_string(), chunk[end + 3..].trim().to_string()));
        }
    }
    out
}

/// The full PM competency matrix: areas → competencies → skills.
/// Parsed from pm_matrix_data.md (`# area`, `## competency`,
/// `- skill | score [| !priority]`).
pub fn matrix() -> Vec<MatrixArea> {
    let theory = parse_theory();
    let mut areas: Vec<MatrixArea> = Vec::new();
    for line in include_str!("pm_matrix_data.md").lines() {
        let line = line.trim_end();
        if let Some(name) = line.strip_prefix("# ") {
            areas.push(MatrixArea { name: name.trim().to_string(), competencies: Vec::new() });
        } else if let Some(name) = line.strip_prefix("## ") {
            let name = name.trim().to_string();
            let th = theory.iter().find(|(n, _)| *n == name)
                .map(|(_, t)| t.clone()).unwrap_or_default();
            if let Some(a) = areas.last_mut() {
                a.competencies.push(MatrixCompetency { name, theory: th, skills: Vec::new() });
            }
        } else if let Some(rest) = line.strip_prefix("- ") {
            let mut parts = rest.split('|');
            let name = parts.next().unwrap_or("").trim().to_string();
            let score: i32 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
            let priority = parts.next().map(|p| p.trim() == "!").unwrap_or(false);
            if name.is_empty() { continue; }
            if let Some(c) = areas.last_mut().and_then(|a| a.competencies.last_mut()) {
                c.skills.push(MatrixSkill { name, score, priority });
            }
        }
    }
    areas
}

/// Practice exercises as `(competency_name, case_title, case_description)`.
pub fn seed_cases() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("Изучение рынка", "Провести 5 проблемных интервью",
         "Выбери продукт, которым пользуешься. Сформулируй гипотезу о проблеме и проведи 5 интервью по The Mom Test — без наводящих вопросов. Зафиксируй факты о реальном поведении, а не мнения."),
        ("Изучение рынка", "Построить JTBD и CJM",
         "Для знакомого продукта опиши 3 Job Stories в формате «Когда… я хочу… чтобы…» и нарисуй Customer Journey Map с шагами, болями и эмоциями пользователя."),
        ("Приоритизация задач", "Приоритизировать бэклог по RICE",
         "Возьми 8 продуктовых идей. Оцени каждую по Reach, Impact, Confidence, Effort и посчитай RICE-score. Объясни, почему топ-3 победили, и что осталось «за бортом»."),
        ("Приоритизация задач", "Разложить фичи по модели Kano",
         "Список из 6 фич распредели на Basic / Performance / Excitement. Объясни, какие нельзя не сделать, а какие дадут «вау-эффект»."),
        ("Основные метрики продукта", "Выбрать North Star Metric",
         "Для выбранного продукта предложи North Star Metric и обоснуй, почему она отражает ценность для пользователя, а не тщеславную (vanity) метрику. Разложи её на воронку AARRR."),
        ("Основные метрики продукта", "Посчитать юнит-экономику",
         "Даны CAC = 1200₽, средний чек 800₽/мес, churn 8%/мес. Посчитай LTV, LTV/CAC и payback period. Здоров ли бизнес? Что улучшать в первую очередь?"),
        ("Сроки и планирование", "Собрать роадмап Now/Next/Later",
         "Составь outcome-based роадмап на 3 горизонта. Для каждого пункта укажи бизнес-цель и метрику, а не только фичу. Подготовь, что НЕ делаем и почему."),
        ("Работа со стейкхолдерами", "Аргументированно сказать «нет»",
         "CEO просит срочную фичу вне роадмапа. Напиши ответ: что подвинется, какой trade-off, какие данные за приоритет. Цель — не отказ, а согласование (alignment)."),
        ("Постановка задач", "Написать PRD на фичу",
         "Выбери фичу и напиши мини-PRD: проблема, аудитория, user stories, acceptance criteria (Given/When/Then), метрики успеха, edge cases."),
        ("Инструменты тестирования", "Спроектировать A/B-тест",
         "Гипотеза: новая формулировка CTA повысит конверсию регистрации. Опиши метрику, MDE, размер выборки, длительность, критерий принятия решения (p < 0.05) и риски."),
        ("Инструменты тестирования", "Разобрать ошибки эксперимента",
         "Тест показал «значимый» рост через 1 день. Найди проблемы: peeking, маленькая выборка, ошибка Type I, сезонность. Как провести корректно?"),
        ("Каналы привлечения", "Составить GTM-план запуска",
         "Для новой фичи опиши positioning, messaging, каналы, сегмент запуска и launch-чеклист (документация, support, метрики, rollback plan)."),
        ("Презентация", "Подготовить elevator pitch",
         "Опиши продукт за 30 секунд: проблема → решение → почему вы. Затем сожми его до executive summary на 5 строк."),
        ("Конкурентный анализ", "Собрать feature-матрицу",
         "Выбери 3 конкурентов и продукт. Построй feature matrix, сделай SWOT по своему продукту и сформулируй уникальное позиционирование."),
        ("Стратегия продукта", "Сформулировать vision и OKR",
         "Напиши product vision на 3 года и один квартальный OKR: Objective + 3 измеримых Key Results. Проверь, что KR — это результаты, а не задачи."),
        ("Инструменты сбора и анализа данных", "Написать запрос для когорт",
         "Дана таблица events(user_id, event, ts). Напиши SQL, считающий retention по неделям регистрации (когортный анализ) с GROUP BY и оконными функциями."),
        ("Agile и Lean", "Провести sprint planning",
         "Свёрстай спринт на 2 недели: выбери задачи из бэклога по приоритету и capacity команды, опиши sprint goal и определи Definition of Done."),
        ("UI", "Нарисовать user flow и wireframe",
         "Выбери сценарий (например, онбординг). Нарисуй user flow по шагам и низкодетальный wireframe ключевого экрана. Где можно срезать шаги?"),
        ("Генерация и тестирование гипотез", "Проверить Product-Market Fit",
         "Опиши, как измеришь PMF для продукта: Sean Ellis Test (>40%), retention-кривая на плато, доля органического роста. Что делать, если PMF нет?"),
        ("Монетизация и бизнес-модель", "Выбрать модель монетизации",
         "Для продукта сравни freemium, подписку и transaction fee. Какую выбрать и почему? Предложи value-based цену и обоснуй её."),
        ("Удержание и возвращаемость", "Найти aha-moment и growth loop",
         "Определи activation-событие («aha-moment») продукта и спроектируй один growth loop (виральный, контентный или платный). Какую метрику он двигает?"),
    ]
}
