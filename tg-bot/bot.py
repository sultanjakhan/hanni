"""Hanni Jobs TG Bot — collect vacancies, manage pipeline."""
import os
import re
import asyncio
import logging
from aiogram import Bot, Dispatcher, F
from aiogram.types import Message, CallbackQuery, InlineKeyboardMarkup, InlineKeyboardButton
from aiogram.filters import Command

import db
import parsers

TOKEN = os.environ.get("BOT_TOKEN", "")
ALLOWED_USER = int(os.environ.get("ALLOWED_USER", "0"))

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s")
log = logging.getLogger("hanni-bot")

bot = Bot(token=TOKEN)
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
    return ALLOWED_USER == 0 or msg.from_user.id == ALLOWED_USER


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


@dp.message(F.text)
async def handle_url(msg: Message):
    if not is_allowed(msg):
        return
    urls = URL_RE.findall(msg.text)
    if not urls:
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
        await msg.answer(card_text(v), parse_mode="HTML", reply_markup=stage_keyboard(vid))


@dp.callback_query(F.data.startswith("stage:"))
async def handle_stage(cb: CallbackQuery):
    _, vid_str, stage = cb.data.split(":")
    vid = int(vid_str)
    await db.update_stage(vid, stage)
    v = await db.get_vacancy(vid)
    if v:
        await cb.message.edit_text(card_text(v), parse_mode="HTML", reply_markup=stage_keyboard(vid))
    await cb.answer(f"✅ {STAGES.get(stage, stage)}")


async def main():
    await db.init_db()
    log.info("Bot starting...")
    await dp.start_polling(bot)


if __name__ == "__main__":
    asyncio.run(main())
