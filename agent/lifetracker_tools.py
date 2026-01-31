"""
Life Tracker Tools for Clawd Agent
Uses Vercel API endpoints to add data to Life Tracker
"""
import aiohttp
from datetime import date

# Vercel deployment URL - change this to your deployment
API_URL = "https://life-tracker-gamma-six.vercel.app/api"
USER_ID = "default"  # Can be changed for multi-user support


async def _post(endpoint: str, data: dict) -> dict:
    """Make POST request to Life Tracker API"""
    try:
        async with aiohttp.ClientSession() as session:
            headers = {"Content-Type": "application/json", "X-User-Id": USER_ID}
            async with session.post(f"{API_URL}/{endpoint}", json=data, headers=headers) as resp:
                return await resp.json()
    except Exception as e:
        return {"error": str(e)}


async def _get(endpoint: str, params: dict = None) -> dict:
    """Make GET request to Life Tracker API"""
    try:
        async with aiohttp.ClientSession() as session:
            headers = {"X-User-Id": USER_ID}
            async with session.get(f"{API_URL}/{endpoint}", params=params, headers=headers) as resp:
                return await resp.json()
    except Exception as e:
        return {"error": str(e)}


def register_lifetracker_tools(agent):
    """Register Life Tracker tools with the agent"""

    @agent.tool("add_purchase")
    async def add_purchase(
        amount: float,
        category: str,
        description: str = "",
        date_str: str = None
    ) -> str:
        """
        Add a purchase to Life Tracker (cloud).

        Args:
            amount: Amount spent (in KZT by default)
            category: Category (food, transport, entertainment, health, education, shopping, bills, other)
            description: What was purchased
            date_str: Date in YYYY-MM-DD format (default: today)

        Returns:
            Confirmation message
        """
        result = await _post("purchase", {
            "amount": amount,
            "category": category.lower(),
            "description": description,
            "date": date_str or date.today().isoformat(),
            "source": "clawd"
        })

        if "error" in result:
            return f"❌ {result['error']}"

        return f"✅ Добавлена покупка: {description or category} - {amount:,.0f} KZT"

    @agent.tool("add_time_entry")
    async def add_time_entry(
        activity: str,
        duration_minutes: int,
        category: str = "work",
        productive: bool = True,
        date_str: str = None
    ) -> str:
        """
        Add a time tracking entry to Life Tracker (cloud).

        Args:
            activity: What you were doing
            duration_minutes: How long in minutes
            category: Category (work, learning, exercise, rest, social, entertainment, chores, other)
            productive: Was this productive time?
            date_str: Date in YYYY-MM-DD format (default: today)

        Returns:
            Confirmation message
        """
        result = await _post("time", {
            "activity": activity,
            "duration": duration_minutes,
            "category": category.lower(),
            "productive": productive,
            "date": date_str or date.today().isoformat(),
        })

        if "error" in result:
            return f"❌ {result['error']}"

        hours = duration_minutes // 60
        mins = duration_minutes % 60
        time_str = f"{hours}h {mins}m" if hours > 0 else f"{mins}m"
        return f"✅ Записано время: {activity} - {time_str}"

    @agent.tool("add_goal")
    async def add_goal(
        title: str,
        description: str = "",
        category: str = "personal",
        target_date: str = None
    ) -> str:
        """
        Add a new goal to Life Tracker (cloud).

        Args:
            title: Goal title
            description: Goal description
            category: Category (health, career, finance, personal, learning, other)
            target_date: Target completion date in YYYY-MM-DD format

        Returns:
            Confirmation message
        """
        result = await _post("goal", {
            "title": title,
            "description": description,
            "category": category.lower(),
            "targetDate": target_date,
        })

        if "error" in result:
            return f"❌ {result['error']}"

        return f"✅ Создана цель: {title}"

    @agent.tool("add_note")
    async def add_note(
        title: str,
        content: str,
        tags: list = None
    ) -> str:
        """
        Add a note to Life Tracker (cloud).

        Args:
            title: Note title
            content: Note content (can be markdown)
            tags: List of tags

        Returns:
            Confirmation message
        """
        result = await _post("note", {
            "title": title,
            "content": content,
            "tags": tags or [],
        })

        if "error" in result:
            return f"❌ {result['error']}"

        return f"✅ Создана заметка: {title}"

    @agent.tool("get_today_stats")
    async def get_today_stats() -> str:
        """
        Get today's statistics from Life Tracker (cloud).

        Returns:
            Summary of today's purchases, time, and activities
        """
        result = await _get("stats", {"days": "1"})

        if "error" in result:
            return f"❌ {result['error']}"

        today = result.get("today", {})
        total = result.get("total", {})

        summary = f"📊 Статистика за сегодня:\n\n"
        summary += f"💰 Потрачено: {today.get('spent', 0):,.0f} KZT\n"

        minutes = today.get('minutes', 0)
        hours = minutes // 60
        mins = minutes % 60
        summary += f"⏱️ Время: {hours}h {mins}m\n"
        summary += f"\n🎯 Активных целей: {total.get('activeGoals', 0)}"

        return summary

    @agent.tool("get_spending_summary")
    async def get_spending_summary(days: int = 30) -> str:
        """
        Get spending summary for the last N days from Life Tracker (cloud).

        Args:
            days: Number of days to look back (default: 30)

        Returns:
            Spending summary by category
        """
        result = await _get("stats", {"days": str(days)})

        if "error" in result:
            return f"❌ {result['error']}"

        total = result.get("total", {})
        by_category = result.get("spendingByCategory", {})

        total_spent = total.get("spent", 0)

        if total_spent == 0:
            return f"Нет покупок за последние {days} дней"

        summary = f"💰 Расходы за {days} дней: {total_spent:,.0f} KZT\n\n"
        summary += "По категориям:\n"

        for cat, amount in sorted(by_category.items(), key=lambda x: -x[1]):
            pct = (amount / total_spent) * 100 if total_spent > 0 else 0
            summary += f"  {cat}: {amount:,.0f} KZT ({pct:.1f}%)\n"

        return summary

    print("✅ Life Tracker tools (cloud): add_purchase, add_time_entry, add_goal, add_note, get_today_stats, get_spending_summary")
