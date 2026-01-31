"""
Google API Tools for Clawd Agent
- Google Calendar
- Google Drive (coming soon)
- Gmail (coming soon)
"""
import os
import pickle
from datetime import datetime, timedelta
from pathlib import Path
from typing import List, Dict, Optional

from google.auth.transport.requests import Request
from google.oauth2.credentials import Credentials
from google_auth_oauthlib.flow import InstalledAppFlow
from googleapiclient.discovery import build

# Directory for credentials and tokens
GOOGLE_DIR = Path(__file__).parent / "google_auth"
GOOGLE_DIR.mkdir(exist_ok=True)

# OAuth scopes - what we can access
SCOPES = [
    'https://www.googleapis.com/auth/calendar',           # Full calendar access
    'https://www.googleapis.com/auth/calendar.events',    # Events
    # Add more scopes as needed:
    # 'https://www.googleapis.com/auth/gmail.readonly',   # Gmail read
    # 'https://www.googleapis.com/auth/drive.readonly',   # Drive read
]


class GoogleAuth:
    """Handle Google OAuth authentication"""

    def __init__(self, credentials_file: str = None):
        self.credentials_file = credentials_file or str(GOOGLE_DIR / "credentials.json")
        self.token_file = str(GOOGLE_DIR / "token.pickle")
        self.creds = None

    def authenticate(self) -> bool:
        """Authenticate with Google. Returns True if successful."""
        # Check for existing token
        if os.path.exists(self.token_file):
            with open(self.token_file, 'rb') as token:
                self.creds = pickle.load(token)

        # If no valid creds, need to login
        if not self.creds or not self.creds.valid:
            if self.creds and self.creds.expired and self.creds.refresh_token:
                self.creds.refresh(Request())
            else:
                if not os.path.exists(self.credentials_file):
                    return False

                flow = InstalledAppFlow.from_client_secrets_file(
                    self.credentials_file, SCOPES
                )
                self.creds = flow.run_local_server(port=0)

            # Save token for next time
            with open(self.token_file, 'wb') as token:
                pickle.dump(self.creds, token)

        return True

    def get_credentials(self):
        """Get authenticated credentials"""
        if not self.creds:
            self.authenticate()
        return self.creds


# Global auth instance
_auth = None

def get_auth() -> GoogleAuth:
    global _auth
    if _auth is None:
        _auth = GoogleAuth()
    return _auth


# === Calendar Tools ===

def calendar_auth_status() -> str:
    """Check if Google Calendar is authenticated"""
    auth = get_auth()
    if os.path.exists(auth.token_file):
        return "Authenticated ✅"
    elif os.path.exists(auth.credentials_file):
        return "Credentials found, need to authenticate. Run calendar_login()"
    else:
        return "No credentials.json found. Please add it to agent/google_auth/"


def calendar_login() -> str:
    """Authenticate with Google Calendar (opens browser)"""
    auth = get_auth()
    if not os.path.exists(auth.credentials_file):
        return f"Error: credentials.json not found at {auth.credentials_file}"

    try:
        if auth.authenticate():
            return "Successfully authenticated with Google Calendar! ✅"
        else:
            return "Authentication failed"
    except Exception as e:
        return f"Authentication error: {e}"


def get_calendar_events(days: int = 7, max_results: int = 10) -> str:
    """Get upcoming calendar events"""
    auth = get_auth()
    if not auth.authenticate():
        return "Not authenticated. Run calendar_login() first."

    try:
        service = build('calendar', 'v3', credentials=auth.creds)

        now = datetime.utcnow().isoformat() + 'Z'
        end = (datetime.utcnow() + timedelta(days=days)).isoformat() + 'Z'

        events_result = service.events().list(
            calendarId='primary',
            timeMin=now,
            timeMax=end,
            maxResults=max_results,
            singleEvents=True,
            orderBy='startTime'
        ).execute()

        events = events_result.get('items', [])

        if not events:
            return f"No events in the next {days} days."

        result = []
        for event in events:
            start = event['start'].get('dateTime', event['start'].get('date'))
            summary = event.get('summary', 'No title')
            result.append(f"• {start[:16]} - {summary}")

        return f"Events (next {days} days):\n" + "\n".join(result)

    except Exception as e:
        return f"Error getting events: {e}"


def create_calendar_event(
    summary: str,
    start_time: str,
    end_time: str = None,
    description: str = None
) -> str:
    """
    Create a calendar event

    Args:
        summary: Event title
        start_time: Start time (ISO format: 2026-01-30T10:00:00)
        end_time: End time (ISO format, optional - defaults to 1 hour after start)
        description: Event description (optional)
    """
    auth = get_auth()
    if not auth.authenticate():
        return "Not authenticated. Run calendar_login() first."

    try:
        service = build('calendar', 'v3', credentials=auth.creds)

        # Parse start time
        if 'T' not in start_time:
            start_time = start_time + 'T09:00:00'

        # Default end time: 1 hour after start
        if not end_time:
            start_dt = datetime.fromisoformat(start_time)
            end_dt = start_dt + timedelta(hours=1)
            end_time = end_dt.isoformat()

        event = {
            'summary': summary,
            'start': {
                'dateTime': start_time,
                'timeZone': 'Asia/Almaty',  # Adjust timezone as needed
            },
            'end': {
                'dateTime': end_time,
                'timeZone': 'Asia/Almaty',
            },
        }

        if description:
            event['description'] = description

        created = service.events().insert(calendarId='primary', body=event).execute()

        return f"Event created: {summary} at {start_time}\nLink: {created.get('htmlLink')}"

    except Exception as e:
        return f"Error creating event: {e}"


def delete_calendar_event(event_id: str) -> str:
    """Delete a calendar event by ID"""
    auth = get_auth()
    if not auth.authenticate():
        return "Not authenticated. Run calendar_login() first."

    try:
        service = build('calendar', 'v3', credentials=auth.creds)
        service.events().delete(calendarId='primary', eventId=event_id).execute()
        return f"Event {event_id} deleted."
    except Exception as e:
        return f"Error deleting event: {e}"


# === Tool Registry for Agent ===

GOOGLE_TOOLS = {
    "calendar_status": {
        "func": calendar_auth_status,
        "description": "Check Google Calendar authentication status",
        "parameters": {}
    },
    "calendar_login": {
        "func": calendar_login,
        "description": "Authenticate with Google Calendar (opens browser)",
        "parameters": {}
    },
    "get_calendar_events": {
        "func": get_calendar_events,
        "description": "Get upcoming calendar events",
        "parameters": {
            "type": "object",
            "properties": {
                "days": {"type": "integer", "description": "Number of days to look ahead (default 7)"},
                "max_results": {"type": "integer", "description": "Maximum events to return (default 10)"}
            }
        }
    },
    "create_calendar_event": {
        "func": create_calendar_event,
        "description": "Create a new calendar event",
        "parameters": {
            "type": "object",
            "properties": {
                "summary": {"type": "string", "description": "Event title"},
                "start_time": {"type": "string", "description": "Start time in ISO format (2026-01-30T10:00:00)"},
                "end_time": {"type": "string", "description": "End time (optional)"},
                "description": {"type": "string", "description": "Event description (optional)"}
            },
            "required": ["summary", "start_time"]
        }
    }
}


def register_google_tools(agent):
    """Register Google tools with an agent"""
    for name, tool in GOOGLE_TOOLS.items():
        agent.register_tool(
            name=name,
            description=tool["description"],
            func=tool["func"],
            parameters=tool.get("parameters")
        )


# Quick test
if __name__ == "__main__":
    print("Google Tools Status:")
    print(calendar_auth_status())
