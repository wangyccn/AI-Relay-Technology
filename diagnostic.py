#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Quick diagnostic tool for CCR application
Checks common issues and provides recommendations
"""
import requests
import json
import sys
import io
from datetime import datetime, timedelta

# Fix Windows console encoding
if sys.platform == 'win32':
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

API_BASE = "http://127.0.0.1:8787"

def check_api_connectivity():
    """Check if API is accessible"""
    try:
        response = requests.get(f"{API_BASE}/api/stats", timeout=5)
        if response.status_code == 200:
            print("✅ API connectivity: OK")
            return True
        else:
            print(f"⚠️  API returned status {response.status_code}")
            return False
    except requests.exceptions.ConnectionError:
        print("❌ API connectivity: FAILED - Cannot connect to API")
        print("   Suggestion: Check if the application is running")
        return False
    except Exception as e:
        print(f"❌ API connectivity: ERROR - {e}")
        return False

def check_recent_errors():
    """Check for recent errors in logs"""
    try:
        response = requests.get(f"{API_BASE}/api/logs", params={
            'level': 'error',
            'limit': 50
        }, timeout=5)

        if response.status_code == 200:
            logs = response.json().get('logs', [])

            if not logs:
                print("✅ Recent errors: None found")
                return True

            # Count errors in last hour
            now = datetime.now().timestamp()
            one_hour_ago = now - 3600
            recent_errors = [log for log in logs if log.get('timestamp', 0) > one_hour_ago]

            if recent_errors:
                print(f"⚠️  Recent errors: {len(recent_errors)} in last hour")

                # Group by source
                by_source = {}
                for log in recent_errors:
                    source = log.get('source', 'unknown')
                    by_source[source] = by_source.get(source, 0) + 1

                print("   Error breakdown:")
                for source, count in sorted(by_source.items(), key=lambda x: -x[1]):
                    print(f"     - {source}: {count}")

                # Show most recent error
                latest = recent_errors[0]
                print(f"\n   Latest error:")
                print(f"     Source: {latest.get('source')}")
                print(f"     Message: {latest.get('message', '')[:100]}")

                return False
            else:
                print(f"✅ Recent errors: {len(logs)} total, but none in last hour")
                return True

    except Exception as e:
        print(f"❌ Error check failed: {e}")
        return False

def check_panic_logs():
    """Check for panic logs"""
    try:
        response = requests.get(f"{API_BASE}/api/logs", params={
            'source': 'panic',
            'limit': 10
        }, timeout=5)

        if response.status_code == 200:
            logs = response.json().get('logs', [])

            if not logs:
                print("✅ Panic logs: None found")
                return True
            else:
                print(f"⚠️  Panic logs: {len(logs)} found")
                print("   Suggestion: Review panic logs for critical issues")

                # Show most recent panic
                latest = logs[0]
                print(f"\n   Latest panic:")
                print(f"     Message: {latest.get('message', '')[:150]}")

                return False

    except Exception as e:
        print(f"❌ Panic check failed: {e}")
        return False

def check_log_volume():
    """Check log volume"""
    try:
        response = requests.get(f"{API_BASE}/api/logs", params={
            'limit': 1000
        }, timeout=5)

        if response.status_code == 200:
            data = response.json()
            total = data.get('total', 0)

            print(f"ℹ️  Total logs: {total}")

            if total > 100000:
                print("   ⚠️  Warning: Large number of logs")
                print("   Suggestion: Consider implementing log cleanup")
            elif total > 10000:
                print("   ℹ️  Log volume is moderate")
            else:
                print("   ✅ Log volume is healthy")

            return True

    except Exception as e:
        print(f"❌ Log volume check failed: {e}")
        return False

def check_streaming_errors():
    """Check for streaming-related errors"""
    try:
        response = requests.get(f"{API_BASE}/api/logs", params={
            'source': 'openai',
            'level': 'error',
            'limit': 50
        }, timeout=5)

        if response.status_code == 200:
            logs = response.json().get('logs', [])

            # Filter for streaming-related errors
            stream_errors = [log for log in logs if 'stream' in log.get('message', '').lower()]

            if not stream_errors:
                print("✅ Streaming errors: None found")
                return True
            else:
                print(f"⚠️  Streaming errors: {len(stream_errors)} found")

                # Check for JSON parse errors
                json_errors = [log for log in stream_errors if 'parse' in log.get('message', '').lower()]
                if json_errors:
                    print(f"   - JSON parse errors: {len(json_errors)}")
                    print("   Suggestion: Check upstream API response format")

                return False

    except Exception as e:
        print(f"❌ Streaming error check failed: {e}")
        return False

def check_glm_support():
    """Check if GLM reasoning_content support is working"""
    try:
        response = requests.get(f"{API_BASE}/api/logs", params={
            'source': 'openai',
            'limit': 100
        }, timeout=5)

        if response.status_code == 200:
            logs = response.json().get('logs', [])

            # Look for reasoning_content mentions
            reasoning_logs = [log for log in logs if 'reasoning' in log.get('message', '').lower()]

            if reasoning_logs:
                print(f"ℹ️  GLM reasoning_content: {len(reasoning_logs)} related logs found")
                print("   ✅ Feature appears to be in use")
            else:
                print("ℹ️  GLM reasoning_content: No related logs found")
                print("   (This is normal if not using GLM models)")

            return True

    except Exception as e:
        print(f"❌ GLM support check failed: {e}")
        return False

def generate_report():
    """Generate diagnostic report"""
    print("=" * 80)
    print("CCR Application Diagnostic Report")
    print("=" * 80)
    print(f"Time: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("=" * 80)
    print()

    checks = [
        ("API Connectivity", check_api_connectivity),
        ("Recent Errors", check_recent_errors),
        ("Panic Logs", check_panic_logs),
        ("Log Volume", check_log_volume),
        ("Streaming Errors", check_streaming_errors),
        ("GLM Support", check_glm_support),
    ]

    results = []
    for name, check_func in checks:
        print(f"\n{name}:")
        print("-" * 80)
        result = check_func()
        results.append((name, result))
        print()

    print("=" * 80)
    print("Summary")
    print("=" * 80)

    passed = sum(1 for _, result in results if result)
    total = len(results)

    for name, result in results:
        status = "✅ PASS" if result else "⚠️  WARN"
        print(f"{status} - {name}")

    print()
    print(f"Overall: {passed}/{total} checks passed")

    if passed == total:
        print("\n✅ All checks passed! Application is healthy.")
    elif passed >= total * 0.7:
        print("\n⚠️  Some issues detected. Review warnings above.")
    else:
        print("\n❌ Multiple issues detected. Immediate attention recommended.")

    print("=" * 80)

    return passed == total

def main():
    """Main entry point"""
    try:
        success = generate_report()
        sys.exit(0 if success else 1)
    except KeyboardInterrupt:
        print("\n\nDiagnostic interrupted by user")
        sys.exit(1)
    except Exception as e:
        print(f"\n\nFatal error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()
