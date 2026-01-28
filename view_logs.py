#!/usr/bin/env python3
"""
Log viewer utility for CCR application
"""
import requests
import json
import sys
from datetime import datetime
import time

API_BASE = "http://127.0.0.1:8787"

def format_log_entry(entry):
    """Format a log entry for display"""
    timestamp = entry.get('timestamp', 'N/A')
    level = entry.get('level', 'N/A')
    source = entry.get('source', 'N/A')
    message = entry.get('message', 'N/A')

    # Color codes
    colors = {
        'ERROR': '\033[91m',  # Red
        'WARN': '\033[93m',   # Yellow
        'INFO': '\033[92m',   # Green
        'DEBUG': '\033[94m',  # Blue
    }
    reset = '\033[0m'

    color = colors.get(level, '')

    return f"{color}[{timestamp}] [{level:5s}] [{source:15s}] {message}{reset}"

def view_logs(level=None, source=None, limit=50, follow=False):
    """View logs from the API"""
    url = f"{API_BASE}/api/logs"
    params = {'limit': limit}

    if level:
        params['level'] = level
    if source:
        params['source'] = source

    try:
        if follow:
            print("Following logs (Ctrl+C to stop)...")
            print("=" * 80)
            last_id = 0
            while True:
                response = requests.get(url, params=params, timeout=5)
                if response.status_code == 200:
                    logs = response.json()

                    # Filter out logs we've already seen
                    new_logs = [log for log in logs if log.get('id', 0) > last_id]

                    if new_logs:
                        for log in new_logs:
                            print(format_log_entry(log))
                            last_id = max(last_id, log.get('id', 0))

                time.sleep(1)
        else:
            response = requests.get(url, params=params, timeout=5)
            if response.status_code == 200:
                logs = response.json()

                if not logs:
                    print("No logs found matching criteria")
                    return

                print(f"Found {len(logs)} log entries:")
                print("=" * 80)

                for log in logs:
                    print(format_log_entry(log))

                print("=" * 80)
            else:
                print(f"Error: HTTP {response.status_code}")
                print(response.text)

    except requests.exceptions.ConnectionError:
        print("Error: Cannot connect to CCR API. Is the application running?")
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nStopped following logs")
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

def view_error_summary():
    """View summary of recent errors"""
    url = f"{API_BASE}/api/logs"
    params = {'level': 'error', 'limit': 100}

    try:
        response = requests.get(url, params=params, timeout=5)
        if response.status_code == 200:
            logs = response.json()

            if not logs:
                print("✅ No errors found!")
                return

            print(f"⚠️  Found {len(logs)} error entries")
            print("=" * 80)

            # Group by source
            by_source = {}
            for log in logs:
                source = log.get('source', 'unknown')
                if source not in by_source:
                    by_source[source] = []
                by_source[source].append(log)

            print("\nErrors by source:")
            for source, source_logs in sorted(by_source.items()):
                print(f"  {source}: {len(source_logs)} errors")

            print("\nMost recent errors:")
            print("-" * 80)
            for log in logs[:10]:
                print(format_log_entry(log))

            print("=" * 80)

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

def view_panic_logs():
    """View panic logs specifically"""
    url = f"{API_BASE}/api/logs"
    params = {'source': 'panic', 'limit': 50}

    try:
        response = requests.get(url, params=params, timeout=5)
        if response.status_code == 200:
            logs = response.json()

            if not logs:
                print("✅ No panic logs found!")
                return

            print(f"⚠️  Found {len(logs)} panic entries")
            print("=" * 80)

            for log in logs:
                print(format_log_entry(log))
                print("-" * 80)

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

def view_stream_logs():
    """View streaming-related logs"""
    url = f"{API_BASE}/api/logs"
    params = {'source': 'openai', 'limit': 100}

    try:
        response = requests.get(url, params=params, timeout=5)
        if response.status_code == 200:
            logs = response.json()

            # Filter for stream-related logs
            stream_logs = [log for log in logs if 'stream' in log.get('message', '').lower()]

            if not stream_logs:
                print("No streaming logs found")
                return

            print(f"Found {len(stream_logs)} streaming-related log entries:")
            print("=" * 80)

            for log in stream_logs:
                print(format_log_entry(log))

            print("=" * 80)

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

def main():
    """Main entry point"""
    if len(sys.argv) < 2:
        print("CCR Log Viewer")
        print("=" * 80)
        print("Usage:")
        print("  python view_logs.py errors          - View error summary")
        print("  python view_logs.py panic           - View panic logs")
        print("  python view_logs.py stream          - View streaming logs")
        print("  python view_logs.py all [limit]     - View all logs")
        print("  python view_logs.py follow          - Follow logs in real-time")
        print("  python view_logs.py level <level>   - View logs by level (error/warn/info/debug)")
        print("  python view_logs.py source <source> - View logs by source")
        print("=" * 80)
        sys.exit(0)

    command = sys.argv[1].lower()

    if command == "errors":
        view_error_summary()
    elif command == "panic":
        view_panic_logs()
    elif command == "stream":
        view_stream_logs()
    elif command == "all":
        limit = int(sys.argv[2]) if len(sys.argv) > 2 else 50
        view_logs(limit=limit)
    elif command == "follow":
        view_logs(follow=True)
    elif command == "level":
        if len(sys.argv) < 3:
            print("Error: Please specify a level (error/warn/info/debug)")
            sys.exit(1)
        view_logs(level=sys.argv[2])
    elif command == "source":
        if len(sys.argv) < 3:
            print("Error: Please specify a source")
            sys.exit(1)
        view_logs(source=sys.argv[2])
    else:
        print(f"Unknown command: {command}")
        sys.exit(1)

if __name__ == "__main__":
    main()
