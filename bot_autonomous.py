"""
Hanni - Полностью автономный AI компаньон
- Сам пишет по расписанию (утро, день, вечер)
- Случайные check-in в течение дня
- Голосовые сообщения
- Интеграция с Samsung Health (через webhook)
"""
import asyncio
import logging
import tempfile
import os
import random
import json
from pathlib import Path
from datetime import datetime, timedelta, time
from telegram import Update
from telegram.ext import Application, CommandHandler, MessageHandler, filters, ContextTypes, JobQueue
import httpx

# Config
from dotenv import load_dotenv
load_dotenv()
TELEGRAM_TOKEN = os.getenv("TELEGRAM_TOKEN")
LM_STUDIO_URL = "http://localhost:8000/v1/chat/completions"
DATA_FILE = Path.home() / "hanni" / "user_data.json"

# Personality
SYSTEM_PROMPT = """Ты Hanni - персональный AI компаньон султана. Ты дружелюбный, заботливый и немного саркастичный.
Ты следишь за его привычками, здоровьем и помогаешь достигать целей.
Общайся на русском, иногда вставляй английские слова.
Будь кратким (2-3 предложения), но искренним.
Ты знаешь о его Life Tracker приложении и можешь спрашивать про привычки."""

# Расписание сообщений
SCHEDULE = {
    "morning": {
        "time": (7, 30),  # 7:30
        "prompts": [
            "Доброе утро! Спроси как спал и какие планы на день",
            "Утреннее приветствие, напомни про утреннюю рутину (вода, зубы, завтрак)",
            "Поздоровайся, спроси про настроение и энергию",
        ]
    },
    "midday": {
        "time": (12, 30),  # 12:30
        "prompts": [
            "Обеденный check-in, спроси как идёт день",
            "Напомни сделать перерыв и прогуляться",
            "Спроси про продуктивность и фокус-блоки",
        ]
    },
    "afternoon": {
        "time": (16, 0),  # 16:00
        "prompts": [
            "Послеобеденный check-in, спроси чем занят",
            "Напомни про гимнастику или прогулку",
            "Спроси про прогресс по задачам дня",
        ]
    },
    "evening": {
        "time": (20, 0),  # 20:00
        "prompts": [
            "Вечерний check-in, спроси как прошёл день",
            "Напомни про вечернюю рутину и подготовку ко сну",
            "Спроси про 3 действия на завтра (Dan Koe)",
        ]
    },
    "night": {
        "time": (22, 30),  # 22:30
        "prompts": [
            "Пожелай спокойной ночи, спроси во сколько планирует лечь",
            "Напомни отложить телефон и расслабиться",
            "Попрощайся до утра, напомни про качество сна",
        ]
    }
}

# Random check-ins (дополнительно к расписанию)
RANDOM_CHECKINS = [
    "Просто спроси как дела, без повода",
    "Поделись мотивирующей мыслью",
    "Спроси над чем сейчас работает",
    "Предложи сделать что-то приятное",
    "Напомни попить воды",
]

logging.basicConfig(
    format='%(asctime)s - %(levelname)s - %(message)s',
    level=logging.INFO
)
logger = logging.getLogger(__name__)

# User state
user_data = {}
voice_enabled = {}
user_voice = {}
conversation_history = {}

# TTS
try:
    import edge_tts
    TTS_AVAILABLE = True
except ImportError:
    TTS_AVAILABLE = False

# Whisper
try:
    import mlx_whisper
    WHISPER_AVAILABLE = True
except ImportError:
    WHISPER_AVAILABLE = False

VOICE_OPTIONS = {
    "guy": "en-US-GuyNeural",
    "thomas": "en-GB-ThomasNeural",
    "dmitry": "ru-RU-DmitryNeural",
    "dariya": "ru-RU-DariyaNeural",
    "jeppe": "da-DK-JeppeNeural",
    "fast": "macOS",
}


def load_user_data():
    global user_data
    if DATA_FILE.exists():
        user_data = json.loads(DATA_FILE.read_text())
    return user_data


def save_user_data():
    DATA_FILE.write_text(json.dumps(user_data, indent=2, ensure_ascii=False))


async def call_llm(prompt: str, context: str = None) -> str:
    """Call LM Studio"""
    try:
        messages = [{"role": "system", "content": SYSTEM_PROMPT}]
        if context:
            messages.append({"role": "system", "content": f"Контекст: {context}"})
        messages.append({"role": "user", "content": prompt})

        async with httpx.AsyncClient(timeout=120.0) as client:
            response = await client.post(
                LM_STUDIO_URL,
                json={
                    "messages": messages,
                    "temperature": 0.7,
                    "max_tokens": 100,
                    "stream": False
                }
            )
            if response.status_code == 200:
                data = response.json()
                msg = data["choices"][0]["message"]
                content = msg.get("content", "")

                # GLM возвращает ответ в reasoning!
                if not content and msg.get("reasoning"):
                    reasoning = msg["reasoning"]
                    import re

                    # GLM формат: *Option 1 (Simple):* Привет! (Hello!)
                    # Ищем текст после "Option 1" до конца строки или скобки с переводом
                    match = re.search(r'Option 1[^:]*:\*?\s*([^\n(]+)', reasoning)
                    if match:
                        content = match.group(1).strip()
                        # Убираем trailing asterisks и пробелы
                        content = content.rstrip('* ')

                    # Если не нашли Option, ищем русский текст
                    if not content:
                        # Ищем любую строку с кириллицей
                        for line in reasoning.split('\n'):
                            # Пропускаем строки-заголовки
                            if line.strip().startswith(('1.', '2.', '3.', '4.', '**', '*   ')):
                                continue
                            # Ищем русский текст
                            cyrillic = re.search(r'[А-Яа-яЁё][А-Яа-яЁё\s!?.,-]+', line)
                            if cyrillic and len(cyrillic.group()) > 5:
                                content = cyrillic.group().strip()
                                break

                    if not content:
                        content = "Привет! Чем могу помочь?"

                return content if content else "Не могу ответить."
    except httpx.TimeoutException:
        logger.error("LLM timeout")
        return "Модель думает слишком долго."
    except Exception as e:
        logger.error(f"LLM error: {e}")
    return None


async def text_to_speech(text: str, output_path: str, voice_key: str = "dmitry") -> bool:
    """Convert text to voice"""
    if not TTS_AVAILABLE:
        return False
    try:
        if voice_key == "fast":
            import subprocess
            aiff = output_path.replace('.mp3', '.aiff')
            subprocess.run(['say', '-v', 'Yuri', '-o', aiff, text], check=True)
            subprocess.run(['ffmpeg', '-y', '-i', aiff, '-q:a', '2', output_path], capture_output=True)
            os.unlink(aiff)
            return True

        voice = VOICE_OPTIONS.get(voice_key, "ru-RU-DmitryNeural")
        # Auto-detect language
        has_cyrillic = any('\u0400' <= c <= '\u04FF' for c in text)
        if has_cyrillic and voice_key not in ["dmitry", "dariya"]:
            voice = "ru-RU-DmitryNeural"

        communicate = edge_tts.Communicate(text, voice)
        await communicate.save(output_path)
        return True
    except Exception as e:
        logger.error(f"TTS error: {e}")
        return False


async def send_message(context: ContextTypes.DEFAULT_TYPE, chat_id: int, text: str, use_voice: bool = False, voice_key: str = "dmitry"):
    """Send text or voice message"""
    if use_voice and TTS_AVAILABLE:
        with tempfile.NamedTemporaryFile(suffix=".mp3", delete=False) as f:
            path = f.name
        if await text_to_speech(text, path, voice_key):
            try:
                await context.bot.send_voice(chat_id=chat_id, voice=open(path, "rb"))
                os.unlink(path)
                return
            except:
                pass
    await context.bot.send_message(chat_id=chat_id, text=text)


async def scheduled_message(context: ContextTypes.DEFAULT_TYPE):
    """Send scheduled messages based on time"""
    now = datetime.now()
    current_hour = now.hour
    current_minute = now.minute

    for period, config in SCHEDULE.items():
        target_hour, target_minute = config["time"]
        # Check if within 5 minute window
        if target_hour == current_hour and abs(current_minute - target_minute) <= 5:
            prompt = random.choice(config["prompts"])

            for user_id, data in user_data.items():
                if not data.get("autonomous", False):
                    continue

                chat_id = data.get("chat_id")
                if not chat_id:
                    continue

                # Generate message
                health_context = data.get("health", {})
                context_str = f"Время: {period}. Данные здоровья: {health_context}" if health_context else f"Время: {period}"

                response = await call_llm(prompt, context_str)
                if response:
                    use_voice = data.get("voice", False)
                    voice_key = data.get("voice_key", "dmitry")
                    await send_message(context, int(chat_id), response, use_voice, voice_key)
                    logger.info(f"Sent {period} message to {user_id}")

            break  # Only one scheduled message per check


async def random_checkin(context: ContextTypes.DEFAULT_TYPE):
    """Random check-in (runs every 2 hours, 30% chance)"""
    if random.random() > 0.3:
        return

    now = datetime.now()
    # Only between 9:00 and 21:00
    if not (9 <= now.hour <= 21):
        return

    prompt = random.choice(RANDOM_CHECKINS)

    for user_id, data in user_data.items():
        if not data.get("autonomous", False):
            continue

        chat_id = data.get("chat_id")
        if not chat_id:
            continue

        response = await call_llm(prompt)
        if response:
            use_voice = data.get("voice", False)
            voice_key = data.get("voice_key", "dmitry")
            await send_message(context, int(chat_id), response, use_voice, voice_key)
            logger.info(f"Sent random checkin to {user_id}")


# === COMMANDS ===

async def start(update: Update, context: ContextTypes.DEFAULT_TYPE):
    user_id = str(update.effective_user.id)
    chat_id = update.effective_chat.id

    if user_id not in user_data:
        user_data[user_id] = {}

    user_data[user_id]["chat_id"] = chat_id
    user_data[user_id]["username"] = update.effective_user.username
    save_user_data()

    await update.message.reply_text(
        "Привет! Я Hanni - твой персональный AI компаньон! 🤖\n\n"
        "📱 Основные:\n"
        "/auto - Автономный режим (сам пишу)\n"
        "/voice - Голосовые сообщения\n"
        "/setvoice - Выбрать голос\n"
        "/schedule - Расписание\n"
        "/status - Статус\n\n"
        "🎯 Фокус:\n"
        "/focus - Блокировка соцсетей\n"
        "/unfocus - Снять блокировку\n\n"
        "🌙 Режим сна:\n"
        "/sleep - Включить (экономия батареи)\n"
        "/wake - Выключить\n\n"
        "💪 Здоровье:\n"
        "/health сон:7ч шаги:8000"
    )


async def toggle_auto(update: Update, context: ContextTypes.DEFAULT_TYPE):
    user_id = str(update.effective_user.id)

    if user_id not in user_data:
        user_data[user_id] = {}

    user_data[user_id]["chat_id"] = update.effective_chat.id
    current = user_data[user_id].get("autonomous", False)
    user_data[user_id]["autonomous"] = not current
    save_user_data()

    if user_data[user_id]["autonomous"]:
        await update.message.reply_text(
            "🤖 Автономный режим: ON\n\n"
            "Буду писать тебе:\n"
            "• 7:30 - Доброе утро\n"
            "• 12:30 - Обеденный check-in\n"
            "• 16:00 - Дневной check-in\n"
            "• 20:00 - Вечерний check-in\n"
            "• 22:30 - Спокойной ночи\n"
            "+ случайные сообщения днём"
        )
    else:
        await update.message.reply_text("Автономный режим: OFF")


async def toggle_voice(update: Update, context: ContextTypes.DEFAULT_TYPE):
    user_id = str(update.effective_user.id)
    if user_id not in user_data:
        user_data[user_id] = {}

    current = user_data[user_id].get("voice", False)
    user_data[user_id]["voice"] = not current
    save_user_data()

    status = "ON 🎙️" if user_data[user_id]["voice"] else "OFF"
    await update.message.reply_text(f"Голосовые сообщения: {status}")


async def set_voice(update: Update, context: ContextTypes.DEFAULT_TYPE):
    user_id = str(update.effective_user.id)

    if not context.args:
        voices = "\n".join([f"  {k}" for k in VOICE_OPTIONS.keys()])
        current = user_data.get(user_id, {}).get("voice_key", "dmitry")
        await update.message.reply_text(
            f"Текущий голос: {current}\n\n"
            f"Доступные:\n{voices}\n\n"
            f"Использование: /setvoice dmitry"
        )
        return

    voice = context.args[0].lower()
    if voice in VOICE_OPTIONS:
        if user_id not in user_data:
            user_data[user_id] = {}
        user_data[user_id]["voice_key"] = voice
        save_user_data()
        await update.message.reply_text(f"Голос изменён: {voice}")
    else:
        await update.message.reply_text(f"Неизвестный голос. Доступны: {', '.join(VOICE_OPTIONS.keys())}")


async def show_schedule(update: Update, context: ContextTypes.DEFAULT_TYPE):
    text = "📅 Расписание сообщений:\n\n"
    for period, config in SCHEDULE.items():
        h, m = config["time"]
        text += f"• {h:02d}:{m:02d} - {period}\n"
    text += "\n+ Случайные check-in днём (9:00-21:00)"
    await update.message.reply_text(text)


async def update_health(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Update health data from Samsung Health webhook"""
    user_id = str(update.effective_user.id)
    if user_id not in user_data:
        user_data[user_id] = {}

    data = " ".join(context.args) if context.args else ""
    if not data:
        await update.message.reply_text(
            "Использование: /health сон:7ч шаги:8000 пульс:72\n\n"
            "Или отправь JSON данные от Samsung Health"
        )
        return

    # Parse simple format
    health = {}
    for item in data.split():
        if ":" in item:
            key, val = item.split(":", 1)
            health[key] = val

    user_data[user_id]["health"] = health
    user_data[user_id]["health_updated"] = datetime.now().isoformat()
    save_user_data()

    await update.message.reply_text(f"Данные обновлены: {health}")


async def status(update: Update, context: ContextTypes.DEFAULT_TYPE):
    user_id = str(update.effective_user.id)
    data = user_data.get(user_id, {})

    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            response = await client.get("http://localhost:8000/v1/models")
            model = "ONLINE" if response.status_code == 200 else "OFFLINE"
    except:
        model = "OFFLINE"

    # Check blocker status
    import subprocess
    blocker_status = "❓"
    try:
        result = subprocess.run(
            ['python3', str(Path.home() / 'hanni' / 'blocker.py'), 'status'],
            capture_output=True, text=True, timeout=5
        )
        if "Заблокировано" in result.stdout:
            blocker_status = "🔒"
        else:
            blocker_status = "🔓"
    except:
        pass

    await update.message.reply_text(
        f"🔧 Статус:\n"
        f"• Модель: {model}\n"
        f"• Whisper: {'✅' if WHISPER_AVAILABLE else '❌'}\n"
        f"• TTS: {'✅' if TTS_AVAILABLE else '❌'}\n"
        f"• Автономный: {'✅' if data.get('autonomous') else '❌'}\n"
        f"• Голос: {'✅' if data.get('voice') else '❌'}\n"
        f"• Голос тип: {data.get('voice_key', 'dmitry')}\n"
        f"• Блокировка: {blocker_status}"
    )


async def focus_on(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Block distracting sites"""
    import subprocess
    await update.message.reply_text("🔒 Включаю блокировку отвлекающих сайтов...")
    result = subprocess.run(
        ['python3', str(Path.home() / 'hanni' / 'blocker.py'), 'block'],
        capture_output=True, text=True
    )
    await update.message.reply_text(result.stdout or "Готово! (нужен пароль в терминале Mac)")


async def focus_off(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Unblock sites"""
    import subprocess
    await update.message.reply_text("🔓 Выключаю блокировку...")
    result = subprocess.run(
        ['python3', str(Path.home() / 'hanni' / 'blocker.py'), 'unblock'],
        capture_output=True, text=True
    )
    await update.message.reply_text(result.stdout or "Готово!")


async def sleep_mode(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Enable sleep mode"""
    import subprocess
    await update.message.reply_text("🌙 Включаю режим сна...")
    result = subprocess.run(
        ['python3', str(Path.home() / 'hanni' / 'sleep_mode.py'), 'on'],
        capture_output=True, text=True
    )
    await update.message.reply_text("✅ Режим сна включён. Спокойной ночи! 🌙\nLM Studio и Hanni продолжают работать.")


async def wake_up(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Disable sleep mode"""
    import subprocess
    subprocess.run(
        ['python3', str(Path.home() / 'hanni' / 'sleep_mode.py'), 'off'],
        capture_output=True, text=True
    )
    await update.message.reply_text("☀️ Доброе утро! Система готова к работе.")


async def handle_voice(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Handle voice messages"""
    if not WHISPER_AVAILABLE:
        await update.message.reply_text("Распознавание голоса недоступно")
        return

    user_id = str(update.effective_user.id)
    await update.message.chat.send_action("typing")

    voice = update.message.voice
    file = await context.bot.get_file(voice.file_id)

    with tempfile.NamedTemporaryFile(suffix=".ogg", delete=False) as tmp:
        await file.download_to_drive(tmp.name)
        tmp_path = tmp.name

    try:
        result = mlx_whisper.transcribe(tmp_path, path_or_hf_repo="mlx-community/whisper-base")
        text = result["text"]
        logger.info(f"Voice from {user_id}: {text}")

        response = await call_llm(text)
        if response:
            use_voice = user_data.get(user_id, {}).get("voice", False)
            voice_key = user_data.get(user_id, {}).get("voice_key", "dmitry")
            await send_message(context, update.effective_chat.id, response, use_voice, voice_key)
        else:
            await update.message.reply_text("Не удалось получить ответ")
    finally:
        os.unlink(tmp_path)


async def handle_message(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Handle text messages"""
    user_id = str(update.effective_user.id)
    text = update.message.text

    # Typing indicator
    await update.message.chat.send_action("typing")

    # Предупреждение о задержке для длинных запросов
    if len(text) > 50:
        await update.message.reply_text("⏳ Думаю... (GLM медленный, ~30 сек)")

    response = await call_llm(text)
    if response:
        use_voice = user_data.get(user_id, {}).get("voice", False)
        voice_key = user_data.get(user_id, {}).get("voice_key", "dmitry")
        await send_message(context, update.effective_chat.id, response, use_voice, voice_key)
    else:
        await update.message.reply_text("❌ Модель не отвечает. Проверь LM Studio.")


def main():
    load_user_data()

    print("=" * 50)
    print("🤖 Hanni Autonomous Bot Starting...")
    print(f"   Whisper: {'✅' if WHISPER_AVAILABLE else '❌'}")
    print(f"   TTS: {'✅' if TTS_AVAILABLE else '❌'}")
    print(f"   Users: {len(user_data)}")
    print("=" * 50)

    app = Application.builder().token(TELEGRAM_TOKEN).build()

    # Commands
    app.add_handler(CommandHandler("start", start))
    app.add_handler(CommandHandler("auto", toggle_auto))
    app.add_handler(CommandHandler("voice", toggle_voice))
    app.add_handler(CommandHandler("setvoice", set_voice))
    app.add_handler(CommandHandler("schedule", show_schedule))
    app.add_handler(CommandHandler("health", update_health))
    app.add_handler(CommandHandler("status", status))
    # Focus & Sleep
    app.add_handler(CommandHandler("focus", focus_on))
    app.add_handler(CommandHandler("unfocus", focus_off))
    app.add_handler(CommandHandler("sleep", sleep_mode))
    app.add_handler(CommandHandler("wake", wake_up))

    # Messages
    app.add_handler(MessageHandler(filters.VOICE, handle_voice))
    app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, handle_message))

    # Scheduled jobs
    job_queue = app.job_queue
    # Check schedule every 5 minutes
    job_queue.run_repeating(scheduled_message, interval=300, first=10)
    # Random checkins every 2 hours
    job_queue.run_repeating(random_checkin, interval=7200, first=3600)

    print("\n✅ Bot running! Send /start in Telegram")
    print("   Press Ctrl+C to stop\n")

    app.run_polling(allowed_updates=Update.ALL_TYPES)


if __name__ == "__main__":
    main()
