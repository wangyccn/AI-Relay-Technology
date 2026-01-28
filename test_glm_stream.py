#!/usr/bin/env python3
"""
Test script for GLM streaming with reasoning_content field
"""
import requests
import json
import sys

def test_glm_streaming():
    """Test GLM streaming request to verify error logging"""
    url = "http://127.0.0.1:8787/v1/chat/completions"

    headers = {
        "Content-Type": "application/json",
        "Authorization": "Bearer test-token"
    }

    payload = {
        "model": "glm-4-plus",
        "messages": [
            {"role": "user", "content": "Hello, how are you?"}
        ],
        "stream": True,
        "temperature": 0.7
    }

    print("=" * 60)
    print("Testing GLM Streaming Request")
    print("=" * 60)
    print(f"URL: {url}")
    print(f"Payload: {json.dumps(payload, indent=2)}")
    print("=" * 60)

    try:
        response = requests.post(url, headers=headers, json=payload, stream=True, timeout=30)

        print(f"\nResponse Status: {response.status_code}")
        print(f"Response Headers: {dict(response.headers)}")
        print("\nStreaming Response:")
        print("-" * 60)

        chunk_count = 0
        for line in response.iter_lines():
            if line:
                chunk_count += 1
                decoded = line.decode('utf-8')
                print(f"Chunk {chunk_count}: {decoded[:200]}")  # Print first 200 chars

                # Try to parse SSE data
                if decoded.startswith("data: "):
                    data = decoded[6:]  # Remove "data: " prefix
                    if data.strip() != "[DONE]":
                        try:
                            json_data = json.loads(data)
                            # Check for reasoning_content
                            if "choices" in json_data:
                                for choice in json_data["choices"]:
                                    if "delta" in choice:
                                        delta = choice["delta"]
                                        if "reasoning_content" in delta:
                                            print(f"  -> Found reasoning_content: {delta['reasoning_content'][:50]}...")
                                        if "content" in delta:
                                            print(f"  -> Found content: {delta['content'][:50]}...")
                        except json.JSONDecodeError as e:
                            print(f"  -> JSON parse error: {e}")

        print("-" * 60)
        print(f"\nTotal chunks received: {chunk_count}")

    except requests.exceptions.RequestException as e:
        print(f"\nRequest Error: {e}")
        return False
    except Exception as e:
        print(f"\nUnexpected Error: {e}")
        import traceback
        traceback.print_exc()
        return False

    return True

def test_non_streaming():
    """Test non-streaming request for comparison"""
    url = "http://127.0.0.1:8787/v1/chat/completions"

    headers = {
        "Content-Type": "application/json",
        "Authorization": "Bearer test-token"
    }

    payload = {
        "model": "glm-4-plus",
        "messages": [
            {"role": "user", "content": "Say hello"}
        ],
        "stream": False,
        "max_tokens": 50
    }

    print("\n" + "=" * 60)
    print("Testing Non-Streaming Request")
    print("=" * 60)

    try:
        response = requests.post(url, headers=headers, json=payload, timeout=30)
        print(f"Status: {response.status_code}")
        print(f"Response: {json.dumps(response.json(), indent=2)}")
        return True
    except Exception as e:
        print(f"Error: {e}")
        return False

if __name__ == "__main__":
    print("GLM Streaming Test Script")
    print("This script tests the error logging improvements\n")

    # Test streaming
    success1 = test_glm_streaming()

    # Test non-streaming
    success2 = test_non_streaming()

    print("\n" + "=" * 60)
    print("Test Summary")
    print("=" * 60)
    print(f"Streaming test: {'PASSED' if success1 else 'FAILED'}")
    print(f"Non-streaming test: {'PASSED' if success2 else 'FAILED'}")
    print("\nCheck the application logs for detailed error messages!")
    print("=" * 60)

    sys.exit(0 if (success1 or success2) else 1)
