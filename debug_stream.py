#!/usr/bin/env python3
"""Simple debug script for streaming issue"""
import requests
import json

API_KEY = "ccr_ZpejUl0GXRT0FcvnIqNxoKYU0ya1g0F1A1W0bKhqBt"
API_URL = "http://localhost:8787/anthropic/v1/messages"

headers = {
    "Content-Type": "application/json",
    "x-api-key": API_KEY,
    "anthropic-version": "2023-06-01"
}

# Test non-streaming first
print("=== Testing Non-Streaming ===")
payload = {
    "model": "claude-sonnet-4-5-20250929",
    "max_tokens": 100,
    "messages": [{"role": "user", "content": "Say hello"}]
}

response = requests.post(API_URL, headers=headers, json=payload, timeout=60)
print(f"Status: {response.status_code}")
print(f"Response body: {response.text[:500]}")

# Test streaming
print("\n=== Testing Streaming ===")
payload["stream"] = True
response = requests.post(API_URL, headers=headers, json=payload, stream=True, timeout=60)
print(f"Status: {response.status_code}")

chunk_num = 0
for line in response.iter_lines():
    if line:
        chunk_num += 1
        decoded = line.decode('utf-8', errors='replace')
        print(f"[{chunk_num}] {decoded[:200]}")
        if chunk_num > 20:
            break
