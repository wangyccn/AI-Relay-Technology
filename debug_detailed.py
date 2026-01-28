#!/usr/bin/env python3
"""Detailed debug script"""
import requests
import json

API_KEY = "ccr_ZpejUl0GXRT0FcvnIqNxoKYU0ya1g0F1A1W0bKhqBt"
API_URL = "http://localhost:8787/anthropic/v1/messages"

headers = {
    "Content-Type": "application/json",
    "x-api-key": API_KEY,
    "anthropic-version": "2023-06-01"
}

# Test non-streaming
print("=== Non-Streaming Test ===")
payload = {
    "model": "claude-sonnet-4-5-20250929",
    "max_tokens": 100,
    "messages": [{"role": "user", "content": "Say hello"}]
}

response = requests.post(API_URL, headers=headers, json=payload, timeout=60)
print(f"Status: {response.status_code}")

if response.status_code == 200:
    try:
        data = response.json()
        print(f"Response keys: {list(data.keys())}")
        print(f"Full JSON: {json.dumps(data, indent=2, ensure_ascii=False)}")

        # Check for content in different formats
        if "content" in data:
            print(f"\nContent found!")
            for block in data.get("content", []):
                print(f"  Block type: {block.get('type')}")
                if block.get("type") == "text":
                    print(f"  Text: {block.get('text')}")
        elif "choices" in data:
            print(f"\nOpenAI format detected (has 'choices')")
            if data["choices"]:
                choice = data["choices"][0]
                if "message" in choice:
                    msg = choice["message"]
                    print(f"  Message keys: {list(msg.keys())}")
                    print(f"  Content: {msg.get('content', 'N/A')}")
                    print(f"  Reasoning: {msg.get('reasoning_content', 'N/A')[:100]}...")
    except Exception as e:
        print(f"Parse error: {e}")
        print(f"Raw response: {response.text}")
else:
    print(f"Error: {response.text}")
