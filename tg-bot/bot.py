"""Hanni Jobs TG Bot — collect vacancies, manage pipeline."""
import os
import re
import asyncio
import logging
from dotenv import load_dotenv

load_dotenv()

from aiogram import Bot, Dispatcher, F
from aiogram.types import Message, CallbackQuery, InlineKeyboardMarkup, InlineKeyboardButton
from aiogram.filters import Command

import db
import parsers

TOKEN = os.environ.get("BOT_TOKEN", "")
_users_raw = os.environ.get("ALLOWED_USERS", "")
ALLOWED_USERS = [u.strip().lower() for u in _users_raw.split(",") if u.strip()]
HANNI_DB = os.path.expanduser("~/Library/Application Support/Hanni/hanni.db")

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s")
log = logging.getLogger("hanni-bot")

bot = None
dp = Dispatcher()

STAGES = {
    "found": "Найдена",
    "applied": "Откликнулся",
    "interview": "Интервью",
    "offer": "Оффер",
    "rejected": "Отказ",
    "ignored": "Пропущена",
}
URL_RE = re.compile(r"https?://[^\s]+")


async def sync_to_hanni(data: dict):
    """Save vacancy to Hanni's main SQLite DB."""
    if not os.path.exists(HANNI_DB):
        return
    try:
        import aiosqlite
        async with aiosqlite.connect(HANNI_DB) as conn:
            await conn.execute(
                """INSERT OR IGNORE INTO job_vacancies (company, position, url, stage, contact, source, salary, notes, found_at, updated_at)
                   VALUES (?, ?, ?, 'found', ?, ?, ?, ?, datetime('now'), datetime('now'))""",
                (data.get('company', ''), data.get('position', ''), data.get('url', ''),
                 data.get('contact', ''), data.get('source', 'tg-bot'),
                 data.get('salary', ''), data.get('notes', '')),
            )
            await conn.commit()
    except Exception as e:
        log.error(f"Hanni sync failed: {e}")


def card_text(v: dict) -> str:
    stage = STAGES.get(v["stage"], v["stage"])
    lines = [f"<b>{v['company']}</b> — {v['position']}"]
    if v.get("salary"):
        lines.append(f"💰 {v['salary']}")
    if v.get("city"):
        lines.append(f"📍 {v['city']}")
    if v.get("experience"):
        lines.append(f"📋 {v['experience']}")
    if v.get("contact"):
        lines.append(f"👤 {v['contact']}")
    lines.append(f"\n🔹 Этап: <b>{stage}</b>")
    lines.append(f"🔗 <a href=\"{v['url']}\">Открыть</a>")
    return "\n".join(lines)


def stage_keyboard(vacancy_id: int) -> InlineKeyboardMarkup:
    buttons = [
        [InlineKeyboardButton(text="📝 Откликнулся", callback_data=f"stage:{vacancy_id}:applied"),
         InlineKeyboardButton(text="📞 Интервью", callback_data=f"stage:{vacancy_id}:interview")],
        [InlineKeyboardButton(text="✅ Оффер", callback_data=f"stage:{vacancy_id}:offer"),
         InlineKeyboardButton(text="❌ Отказ", callback_data=f"stage:{vacancy_id}:rejected")],
        [InlineKeyboardButton(text="⏭ Пропустить", callback_data=f"stage:{vacancy_id}:ignored")],
    ]
    return InlineKeyboardMarkup(inline_keyboard=buttons)


def is_allowed(msg: Message) -> bool:
    if not ALLOWED_USERS:
        return True
    username = (msg.from_user.username or "").lower()
    return username in ALLOWED_USERS


@dp.message(Command("start"))
async def cmd_start(msg: Message):
    if not is_allowed(msg):
        return
    await msg.answer(
        "👋 Привет! Кидай ссылку на вакансию — я добавлю в таблицу.\n\n"
        "Команды:\n"
        "/list — все вакансии\n"
        "/pipeline — по этапам\n"
        "/help — помощь"
    )


@dp.message(Command("help"))
async def cmd_help(msg: Message):
    if not is_allowed(msg):
        return
    await msg.answer(
        "📖 <b>Команды:</b>\n\n"
        "/list — все вакансии (последние 20)\n"
        "/pipeline — вакансии по этапам\n"
        "/help — эта справка\n\n"
        "<b>Как добавить вакансию:</b>\n"
        "Просто кинь ссылку на вакансию (hh.kz, djinni, linkedin и др.) — "
        "я распарсю и добавлю в таблицу.\n\n"
        "<b>Этапы:</b> Найдена → Откликнулся → Интервью → Оффер/Отказ",
        parse_mode="HTML",
    )


@dp.message(Command("list"))
async def cmd_list(msg: Message):
    if not is_allowed(msg):
        return
    vacancies = await db.list_vacancies()
    if not vacancies:
        await msg.answer("Вакансий пока нет. Кинь ссылку!")
        return
    lines = []
    for v in vacancies[:20]:
        stage = STAGES.get(v["stage"], v["stage"])
        lines.append(f"• <b>{v['company']}</b> — {v['position']} [{stage}]")
    text = f"📋 Вакансий: {len(vacancies)}\n\n" + "\n".join(lines)
    if len(vacancies) > 20:
        text += f"\n\n...и ещё {len(vacancies) - 20}"
    await msg.answer(text, parse_mode="HTML")


@dp.message(Command("pipeline"))
async def cmd_pipeline(msg: Message):
    if not is_allowed(msg):
        return
    vacancies = await db.list_vacancies()
    if not vacancies:
        await msg.answer("Пусто!")
        return
    by_stage = {}
    for v in vacancies:
        by_stage.setdefault(v["stage"], []).append(v)
    lines = []
    for stage_key, label in STAGES.items():
        items = by_stage.get(stage_key, [])
        if items:
            lines.append(f"\n<b>{label}</b> ({len(items)}):")
            for v in items[:5]:
                lines.append(f"  • {v['company']} — {v['position']}")
            if len(items) > 5:
                lines.append(f"  ...и ещё {len(items) - 5}")
    await msg.answer("📊 Pipeline:\n" + "\n".join(lines), parse_mode="HTML")


@dp.message()
async def handle_url(msg: Message):
    if not is_allowed(msg):
        return
    # Extract URLs from text, caption, and entities
    text = msg.text or msg.caption or ''
    urls = URL_RE.findall(text)
    # Also extract from entities (url type and text_link type)
    for entities in [msg.entities or [], msg.caption_entities or []]:
        for ent in entities:
            if ent.type == 'url':
                urls.append(ent.extract_from(text))
            elif ent.type == 'text_link' and ent.url:
                urls.append(ent.url)
    urls = list(dict.fromkeys(urls))  # deduplicate, preserve order
    if not urls:
        if text and not text.startswith('/'):
            await msg.answer("Не нашёл ссылку. Кинь URL вакансии.")
        return

    for url in urls[:3]:
        await msg.answer(f"⏳ Парсю {url}...")
        data = await parsers.parse_url(url.rstrip("/"))
        vid = await db.add_vacancy(data)
        if vid is None:
            await msg.answer(f"⚠️ Уже добавлена: {data.get('position', url)}")
            continue
        v = await db.get_vacancy(vid)
        await sync_to_hanni(data)
        await msg.answer(card_text(v), parse_mode="HTML", reply_markup=stage_keyboard(vid))


@dp.callback_query(F.data.startswith("stage:"))
async def handle_stage(cb: CallbackQuery):
    _, vid_str, stage = cb.data.split(":")
    vid = int(vid_str)
    await db.update_stage(vid, stage)
    v = await db.get_vacancy(vid)
    if v:
        # Sync stage to Hanni
        try:
            import aiosqlite
            if os.path.exists(HANNI_DB):
                async with aiosqlite.connect(HANNI_DB) as conn:
                    await conn.execute("UPDATE job_vacancies SET stage=?, updated_at=datetime('now') WHERE url=?", (stage, v['url']))
                    await conn.commit()
        except Exception as e:
            log.error(f"Hanni stage sync: {e}")
        await cb.message.edit_text(card_text(v), parse_mode="HTML", reply_markup=stage_keyboard(vid))
    await cb.answer(f"✅ {STAGES.get(stage, stage)}")


async def main():
    global bot
    if not TOKEN:
        log.error("BOT_TOKEN not set!")
        return
    bot = Bot(token=TOKEN)
    await db.init_db()
    log.info("Bot starting...")
    await dp.start_polling(bot)


if __name__ == "__main__":
    asyncio.run(main())
