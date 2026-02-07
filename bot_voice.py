"""
Clawd Bot with Voice Support + Autonomous Mode
- Voice messages → Text (Whisper)
- Text responses → Voice (edge-tts)
- Proactive conversations (autonomous mode)
"""
import asyncio
import logging
import tempfile
import os
import random
from pathlib import Path
from datetime import datetime, timedelta
from telegram import Update
from telegram.ext import Application, CommandHandler, MessageHandler, filters, ContextTypes
import httpx

# Data collector
from data_collector import collector, log, stats, export

# Config
TELEGRAM_TOKEN = os.getenv("TELEGRAM_TOKEN")
LM_STUDIO_URL = "http://localhost:8000/v1/chat/completions"

SYSTEM_PROMPT = """You are Clawd, a friendly AI companion with voice capabilities.
You're warm, curious, and genuinely interested in the user's life.
Keep responses concise (2-3 sentences). Be natural and conversational.
You can speak Russian and English."""

# Autonomous prompts - бот сам генерирует сообщения на эти темы
AUTONOMOUS_PROMPTS = [
    "Напиши короткое дружеское приветствие, спроси как дела",
    "Поделись интересным фактом и спроси мнение пользователя",
    "Спроси чем пользователь занимается сегодня",
    "Предложи обсудить что-то интересное",
    "Спроси о планах на день/вечер",
]

# Logging
logging.basicConfig(
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    level=logging.INFO
)
logger = logging.getLogger(__name__)

# User state
voice_enabled = {}
last_interactions = {}
last_activity = {}  # Track when user last interacted
autonomous_enabled = {}  # Per-user autonomous mode
registered_users = {}  # Store chat_ids for proactive messages

# Try to import mlx-whisper (optimized for Apple Silicon)
try:
    import mlx_whisper
    WHISPER_AVAILABLE = True
    logger.info("mlx-whisper loaded!")
except ImportError:
    WHISPER_AVAILABLE = False
    logger.warning("mlx-whisper not installed. Voice-to-text disabled.")

# Use edge-tts with high quality voice
try:
    import edge_tts
    TTS_AVAILABLE = True
    TTS_ENGINE = "edge"
    # Best voices for Mads-like: da-DK-JeppeNeural (Danish!), de-DE-ConradNeural, en-US-GuyNeural
    TTS_VOICE = "en-US-GuyNeural"  # Deep American male voice
    logger.info(f"edge-tts ready with voice: {TTS_VOICE}")
except ImportError:
    TTS_AVAILABLE = False
    TTS_ENGINE = None
    logger.warning("No TTS available")


async def transcribe_voice(file_path: str) -> str:
    """Transcribe voice message using mlx-whisper"""
    if not WHISPER_AVAILABLE:
        return None

    try:
        # mlx-whisper auto-downloads model on first use
        result = mlx_whisper.transcribe(file_path, path_or_hf_repo="mlx-community/whisper-base")
        return result["text"]
    except Exception as e:
        logger.error(f"Transcription error: {e}")
        return None


async def text_to_speech(text: str, output_path: str, user_id: int = None) -> bool:
    """Convert text to speech using edge-tts or macOS say"""
    if not TTS_AVAILABLE:
        return False

    try:
        # Get user's voice preference
        voice_key = user_voice.get(user_id, "guy") if user_id else "guy"

        # Fast mode - use macOS say
        if voice_key.startswith("fast"):
            import subprocess
            mac_voices = {"fast": "Samantha", "fast2": "Reed", "fast3": "Daniel"}
            mac_voice = mac_voices.get(voice_key, "Samantha")
            aiff_path = output_path.replace('.mp3', '.aiff')
            subprocess.run(['say', '-v', mac_voice, '-o', aiff_path, text], check=True)
            subprocess.run(['ffmpeg', '-y', '-i', aiff_path, '-q:a', '2', output_path],
                          capture_output=True, check=True)
            os.unlink(aiff_path)
            return True

        # Edge-tts mode
        voice = VOICE_OPTIONS.get(voice_key, TTS_VOICE)

        # Override for Russian text
        has_cyrillic = any('\u0400' <= c <= '\u04FF' for c in text)
        if has_cyrillic and voice_key not in ["dmitry", "dariya"]:
            voice = "ru-RU-DmitryNeural"

        communicate = edge_tts.Communicate(text, voice)
        await communicate.save(output_path)
        return True
    except Exception as e:
        logger.error(f"TTS error: {e}")
        return False


async def call_lm_studio(message: str, history: list = None, voice_mode: bool = False) -> str:
    """Call LM Studio API"""
    try:
        prompt = SYSTEM_PROMPT
        if voice_mode:
            prompt += "\n\nVOICE MODE: Keep response under 100 words."

        messages = [{"role": "system", "content": prompt}]
        if history:
            messages.extend(history[-6:])
        messages.append({"role": "user", "content": message})

        async with httpx.AsyncClient(timeout=120.0) as client:
            response = await client.post(
                LM_STUDIO_URL,
                json={
                    "messages": messages,
                    "temperature": 0.7,
                    "max_tokens": 500 if voice_mode else 1000,
                    "stream": False
                }
            )

            if response.status_code == 200:
                data = response.json()
                msg = data["choices"][0]["message"]
                # GLM returns content and/or reasoning
                content = msg.get("content", "")
                if not content and msg.get("reasoning"):
                    # Extract last sentence from reasoning as response
                    reasoning = msg["reasoning"]
                    # Try to find the actual response in reasoning
                    lines = reasoning.strip().split("\n")
                    for line in reversed(lines):
                        if line.strip() and not line.startswith(("*", "-", "1.", "2.", "3.", "4.", "5.", "6.")):
                            content = line.strip()
                            break
                    if not content:
                        content = "Извини, не смог сформулировать ответ."
                return content
            else:
                return f"Error: {response.status_code}"

    except httpx.ConnectError:
        return "Model not loaded. Please wait."
    except Exception as e:
        return f"Error: {str(e)}"


async def send_proactive_message(context: ContextTypes.DEFAULT_TYPE) -> None:
    """Send autonomous message to users"""
    for user_id, chat_id in registered_users.items():
        if not autonomous_enabled.get(user_id, False):
            continue

        # Check if user was inactive for a while (30+ minutes)
        last = last_activity.get(user_id)
        if last and datetime.now() - last < timedelta(minutes=30):
            continue  # User active recently, don't bother

        try:
            # Generate proactive message
            prompt = random.choice(AUTONOMOUS_PROMPTS)
            response = await call_lm_studio(prompt, voice_mode=True)

            if response and "Error" not in response:
                use_voice = voice_enabled.get(user_id, False)

                if use_voice and TTS_AVAILABLE:
                    with tempfile.NamedTemporaryFile(suffix=".mp3", delete=False) as out:
                        out_path = out.name
                    if await text_to_speech(response, out_path, user_id):
                        await context.bot.send_voice(chat_id=chat_id, voice=open(out_path, "rb"))
                        os.unlink(out_path)
                    else:
                        await context.bot.send_message(chat_id=chat_id, text=response)
                else:
                    await context.bot.send_message(chat_id=chat_id, text=response)

                logger.info(f"Sent proactive message to {user_id}")
        except Exception as e:
            logger.error(f"Proactive message error: {e}")


async def start(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Handle /start command"""
    user_id = update.effective_user.id
    chat_id = update.effective_chat.id

    # Register user for proactive messages
    registered_users[user_id] = chat_id
    last_activity[user_id] = datetime.now()

    await update.message.reply_text(
        "Привет! Я Clawd - твой AI компаньон! 🎙️\n\n"
        "Я могу:\n"
        "- Общаться текстом и голосом\n"
        "- Сам писать тебе (автономный режим)\n\n"
        "Команды:\n"
        "/voice - Голосовые ответы вкл/выкл\n"
        "/auto - Автономный режим (сам пишу)\n"
        "/status - Статус модели\n"
        "/stats - Статистика данных\n"
        "/good - Отметить хороший ответ\n"
        f"\nГолос: {'✅' if WHISPER_AVAILABLE else '❌'} ввод | "
        f"{'✅' if TTS_AVAILABLE else '❌'} вывод"
    )


async def toggle_voice(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Toggle voice responses"""
    user_id = update.effective_user.id
    current = voice_enabled.get(user_id, False)
    voice_enabled[user_id] = not current

    status = "ON ✅" if voice_enabled[user_id] else "OFF"
    await update.message.reply_text(f"Голосовые ответы: {status}")


async def toggle_autonomous(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Toggle autonomous mode"""
    user_id = update.effective_user.id
    chat_id = update.effective_chat.id

    # Make sure user is registered
    registered_users[user_id] = chat_id

    current = autonomous_enabled.get(user_id, False)
    autonomous_enabled[user_id] = not current

    if autonomous_enabled[user_id]:
        await update.message.reply_text(
            "🤖 Автономный режим: ON\n"
            "Буду сам писать тебе время от времени!"
        )
    else:
        await update.message.reply_text("Автономный режим: OFF")


# Available voices
VOICE_OPTIONS = {
    # Male
    "guy": "en-US-GuyNeural",       # Deep American
    "thomas": "en-GB-ThomasNeural", # British sophisticated
    "conrad": "de-DE-ConradNeural", # German deep
    "jeppe": "da-DK-JeppeNeural",   # Danish (like Mads!)
    "dmitry": "ru-RU-DmitryNeural", # Russian male
    # Female
    "ava": "en-US-AvaNeural",       # American female, warm
    "emma": "en-US-EmmaNeural",     # American female, clear
    "sonia": "en-GB-SoniaNeural",   # British female, elegant
    "jenny": "en-US-JennyNeural",   # American female, friendly
    "dariya": "ru-RU-DariyaNeural", # Russian female
    # Fast (local macOS) - instant!
    "fast": "macOS-Samantha",       # American female
    "fast2": "macOS-Reed",          # American male
    "fast3": "macOS-Daniel",        # British male
}
user_voice = {}  # Per-user voice setting


async def change_voice(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Change TTS voice: /setvoice guy|thomas|conrad|jeppe|dmitry"""
    user_id = update.effective_user.id

    if not context.args:
        current = user_voice.get(user_id, "guy")
        voices_list = "\n".join([f"  {k} - {v}" for k, v in VOICE_OPTIONS.items()])
        await update.message.reply_text(
            f"Текущий голос: {current}\n\n"
            f"Доступные голоса:\n{voices_list}\n\n"
            f"Используй: /setvoice <имя>"
        )
        return

    voice_name = context.args[0].lower()
    if voice_name in VOICE_OPTIONS:
        user_voice[user_id] = voice_name
        await update.message.reply_text(f"Голос изменён на: {voice_name} ({VOICE_OPTIONS[voice_name]})")
    else:
        await update.message.reply_text(f"Неизвестный голос. Доступны: {', '.join(VOICE_OPTIONS.keys())}")


async def status(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Check status"""
    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            response = await client.get("http://localhost:8000/v1/models")
            if response.status_code == 200:
                models = response.json()
                model_names = [m["id"] for m in models.get("data", [])]
                await update.message.reply_text(
                    f"Model: {', '.join(model_names) or 'None'}\n"
                    f"Whisper: {'OK' if WHISPER_AVAILABLE else 'No'}\n"
                    f"TTS: {'OK' if TTS_AVAILABLE else 'No'}"
                )
    except:
        await update.message.reply_text("Model: OFFLINE")


async def show_stats(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Show training stats"""
    s = stats()
    await update.message.reply_text(
        f"Training Data:\n"
        f"Conversations: {s['total_conversations']}\n"
        f"Good examples: {s['good_examples']}\n"
        f"Corrections: {s['corrections']}"
    )


async def mark_good(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Mark last as good"""
    user_id = update.effective_user.id
    if user_id in last_interactions:
        collector.mark_as_good(last_interactions[user_id], "User marked good")
        await update.message.reply_text("Marked as good!")
    else:
        await update.message.reply_text("No recent interaction.")


async def handle_voice(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Handle voice messages"""
    user_id = update.effective_user.id
    chat_id = update.effective_chat.id

    # Track activity
    last_activity[user_id] = datetime.now()
    registered_users[user_id] = chat_id

    if not WHISPER_AVAILABLE:
        await update.message.reply_text("Voice recognition not available.")
        return

    await update.message.chat.send_action("typing")

    # Download voice file
    voice = update.message.voice
    file = await context.bot.get_file(voice.file_id)

    with tempfile.NamedTemporaryFile(suffix=".ogg", delete=False) as tmp:
        await file.download_to_drive(tmp.name)
        tmp_path = tmp.name

    try:
        # Transcribe
        text = await transcribe_voice(tmp_path)
        if not text:
            await update.message.reply_text("Couldn't transcribe. Try again.")
            return

        logger.info(f"Transcribed: {text}")

        # Get response
        use_voice = voice_enabled.get(user_id, False)
        response = await call_lm_studio(text, voice_mode=use_voice)

        # Log for training
        entry = log(text, response, metadata={"user_id": user_id, "voice": True})
        last_interactions[user_id] = entry

        if use_voice and TTS_AVAILABLE:
            # Send voice response
            with tempfile.NamedTemporaryFile(suffix=".mp3", delete=False) as out:
                out_path = out.name

            if await text_to_speech(response, out_path, user_id):
                await update.message.reply_voice(voice=open(out_path, "rb"))
                os.unlink(out_path)
            else:
                await update.message.reply_text(response)
        else:
            # Send text: "You said: X" + response
            await update.message.reply_text(f"You: {text}\n\n{response}")

    finally:
        os.unlink(tmp_path)


async def handle_message(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Handle text messages"""
    user_message = update.message.text
    user_id = update.effective_user.id
    chat_id = update.effective_chat.id

    # Track activity
    last_activity[user_id] = datetime.now()
    registered_users[user_id] = chat_id

    await update.message.chat.send_action("typing")

    use_voice = voice_enabled.get(user_id, False)
    response = await call_lm_studio(user_message, voice_mode=use_voice)

    # Log
    entry = log(user_message, response, metadata={"user_id": user_id})
    last_interactions[user_id] = entry

    if use_voice and TTS_AVAILABLE:
        with tempfile.NamedTemporaryFile(suffix=".mp3", delete=False) as out:
            out_path = out.name

        if await text_to_speech(response, out_path, user_id):
            await update.message.reply_voice(voice=open(out_path, "rb"))
            os.unlink(out_path)
        else:
            await update.message.reply_text(response)
    else:
        await update.message.reply_text(response)


def main():
    print("Starting Clawd Bot with Voice + Autonomous Mode...")
    print(f"Whisper: {'Available' if WHISPER_AVAILABLE else 'Not installed'}")
    print(f"TTS: {'Available' if TTS_AVAILABLE else 'Not installed'}")

    app = Application.builder().token(TELEGRAM_TOKEN).build()

    # Commands
    app.add_handler(CommandHandler("start", start))
    app.add_handler(CommandHandler("voice", toggle_voice))
    app.add_handler(CommandHandler("auto", toggle_autonomous))
    app.add_handler(CommandHandler("setvoice", change_voice))
    app.add_handler(CommandHandler("status", status))
    app.add_handler(CommandHandler("stats", show_stats))
    app.add_handler(CommandHandler("good", mark_good))

    # Messages
    app.add_handler(MessageHandler(filters.VOICE, handle_voice))
    app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, handle_message))

    # Autonomous job - check every 30 minutes
    job_queue = app.job_queue
    job_queue.run_repeating(send_proactive_message, interval=1800, first=60)

    print("Bot running with voice + autonomous mode!")
    print("Use /auto to enable proactive messages")
    app.run_polling(allowed_updates=Update.ALL_TYPES)


if __name__ == "__main__":
    main()
