"""
Clawd Bot - Telegram bot with LM Studio backend + Data Collection for Fine-tuning
"""
import asyncio
import logging
from telegram import Update
from telegram.ext import Application, CommandHandler, MessageHandler, filters, ContextTypes
import httpx

# Data collector for fine-tuning
from data_collector import collector, log, correct, stats, export

# Config
TELEGRAM_TOKEN = "REDACTED"
LM_STUDIO_URL = "http://localhost:8000/v1/chat/completions"

SYSTEM_PROMPT = """You are Clawd, a helpful AI assistant. You are running locally on the user's machine.
Be concise, helpful, and friendly. You can help with coding, browsing, and general tasks."""

# Logging
logging.basicConfig(
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    level=logging.INFO
)
logger = logging.getLogger(__name__)

# Store last interaction for corrections
last_interactions = {}


async def call_lm_studio(message: str, history: list = None) -> str:
    """Call LM Studio API"""
    try:
        messages = [{"role": "system", "content": SYSTEM_PROMPT}]

        if history:
            messages.extend(history[-6:])  # Last 3 exchanges

        messages.append({"role": "user", "content": message})

        async with httpx.AsyncClient(timeout=120.0) as client:
            response = await client.post(
                LM_STUDIO_URL,
                json={
                    "messages": messages,
                    "temperature": 0.7,
                    "max_tokens": 2000,
                    "stream": False
                }
            )

            if response.status_code == 200:
                data = response.json()
                return data["choices"][0]["message"]["content"]
            else:
                return f"LM Studio error: {response.status_code}"

    except httpx.ConnectError:
        return "LM Studio not running. Load model and start server."
    except Exception as e:
        return f"Error: {str(e)}"


async def start(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Handle /start command"""
    await update.message.reply_text(
        "Hey! I'm Clawd Bot (GLM-4.7 powered)\n\n"
        "Commands:\n"
        "/start - This message\n"
        "/status - Check model status\n"
        "/stats - Training data statistics\n"
        "/good - Mark last response as good example\n"
        "/bad <correction> - Correct last response\n"
        "/export - Export training data\n"
        "/clear - Clear conversation history"
    )


async def status(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Check LM Studio status"""
    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            response = await client.get("http://localhost:8000/v1/models")
            if response.status_code == 200:
                models = response.json()
                if models.get("data"):
                    model_names = [m["id"] for m in models["data"]]
                    await update.message.reply_text(
                        f"Status: ONLINE\n"
                        f"Model: {', '.join(model_names)}"
                    )
                else:
                    await update.message.reply_text(
                        "Status: ONLINE\n"
                        "No models loaded."
                    )
            else:
                await update.message.reply_text("Status: ERROR")
    except Exception:
        await update.message.reply_text("Status: OFFLINE")


async def show_stats(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Show training data statistics"""
    s = stats()
    await update.message.reply_text(
        f"Training Data Stats:\n"
        f"Total conversations: {s['total_conversations']}\n"
        f"Good examples: {s['good_examples']}\n"
        f"Corrections: {s['corrections']}\n"
        f"This session: {s['session_interactions']}"
    )


async def mark_good(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Mark last response as good example"""
    user_id = update.effective_user.id
    if user_id in last_interactions:
        entry = last_interactions[user_id]
        collector.mark_as_good(entry, "User marked as good")
        await update.message.reply_text("Marked as good example for training!")
    else:
        await update.message.reply_text("No recent interaction to mark.")


async def mark_bad(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Correct last response"""
    user_id = update.effective_user.id
    correction = " ".join(context.args) if context.args else None

    if not correction:
        await update.message.reply_text("Usage: /bad <correct response>")
        return

    if user_id in last_interactions:
        entry = last_interactions[user_id]
        user_msg = entry["messages"][-2]["content"]  # User message
        original = entry["messages"][-1]["content"]  # Original response
        correct(original, correction, user_msg, "User correction")
        await update.message.reply_text("Correction saved for training!")
    else:
        await update.message.reply_text("No recent interaction to correct.")


async def export_data(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Export training data"""
    path, count = export()
    await update.message.reply_text(
        f"Exported {count} examples to:\n{path}"
    )


async def clear_history(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Clear conversation history"""
    user_id = update.effective_user.id
    context.user_data["history"] = []
    await update.message.reply_text("Conversation history cleared!")


async def handle_message(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Handle incoming messages"""
    user_message = update.message.text
    user_id = update.effective_user.id
    username = update.effective_user.username or "unknown"

    logger.info(f"Message from {username}: {user_message}")

    # Get or create history
    if "history" not in context.user_data:
        context.user_data["history"] = []
    history = context.user_data["history"]

    # Send typing indicator
    await update.message.chat.send_action("typing")

    # Get response from LM Studio
    response = await call_lm_studio(user_message, history)

    # Update history
    history.append({"role": "user", "content": user_message})
    history.append({"role": "assistant", "content": response})

    # Keep only last 10 messages
    if len(history) > 10:
        context.user_data["history"] = history[-10:]

    # Log interaction for training
    entry = log(
        user_message,
        response,
        system_prompt=SYSTEM_PROMPT,
        metadata={"user_id": user_id, "username": username}
    )
    last_interactions[user_id] = entry

    await update.message.reply_text(response)


def main() -> None:
    """Start the bot"""
    print("Starting Clawd Bot with data collection...")
    print(f"Training data: {collector.conversations_file.parent}")

    # Create application
    app = Application.builder().token(TELEGRAM_TOKEN).build()

    # Add handlers
    app.add_handler(CommandHandler("start", start))
    app.add_handler(CommandHandler("status", status))
    app.add_handler(CommandHandler("stats", show_stats))
    app.add_handler(CommandHandler("good", mark_good))
    app.add_handler(CommandHandler("bad", mark_bad))
    app.add_handler(CommandHandler("export", export_data))
    app.add_handler(CommandHandler("clear", clear_history))
    app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, handle_message))

    # Start polling
    print("Bot is running! Press Ctrl+C to stop.")
    app.run_polling(allowed_updates=Update.ALL_TYPES)


if __name__ == "__main__":
    main()
