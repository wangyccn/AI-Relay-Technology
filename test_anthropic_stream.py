#!/usr/bin/env python3
"""
Anthropic Streaming API Test Script

Tests Claude Messages API with streaming support.
Supports both standard requests and anthropic SDK.
"""
import os
import sys
import json
import time
from typing import Optional

try:
    import requests
    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False

try:
    import anthropic
    HAS_ANTHROPIC_SDK = True
except ImportError:
    HAS_ANTHROPIC_SDK = False


# Default configuration
# DEFAULT_API_KEY = os.environ.get("ANTHROPIC_API_KEY", "444f2e9b224443e7bdea16539e7c3475.JRc1XEdfYjhY0k7r")
DEFAULT_API_KEY = os.environ.get("ANTHROPIC_API_KEY", "ccr_ZpejUl0GXRT0FcvnIqNxoKYU0ya1g0F1A1W0bKhqBt")
DEFAULT_MODEL = "claude-sonnet-4-5-20250929"
DEFAULT_API_URL = "http://localhost:8787/anthropic/v1/messages"

# DEFAULT_API_URL = "http://open.bigmodel.cn/api/anthropic/v1/messages"

class Colors:
    """ANSI color codes for terminal output"""
    HEADER = '\033[95m'
    BLUE = '\033[94m'
    CYAN = '\033[96m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    RED = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'
    DIM = '\033[2m'


def print_color(color: str, text: str):
    """Print colored text if terminal supports it"""
    print(f"{color}{text}{Colors.ENDC}")


def print_header(text: str):
    """Print a section header"""
    print("\n" + "=" * 70)
    print_color(Colors.BOLD + Colors.HEADER, text)
    print("=" * 70)


def print_success(text: str):
    """Print success message"""
    print_color(Colors.GREEN, f"✓ {text}")


def print_error(text: str):
    """Print error message"""
    print_color(Colors.RED, f"✗ {text}")


def print_info(text: str):
    """Print info message"""
    print_color(Colors.CYAN, f"ℹ {text}")


class AnthropicStreamTester:
    """Anthropic API streaming tester"""

    def __init__(
        self,
        api_key: str,
        api_url: str = DEFAULT_API_URL,
        model: str = DEFAULT_MODEL
    ):
        self.api_key = api_key
        self.api_url = api_url
        self.model = model
        self.headers = {
            "Content-Type": "application/json",
            "x-api-key": api_key,
            "anthropic-version": "2023-06-01"
        }

    def test_streaming(
        self,
        message: str = "Hello! Please tell me a short joke.",
        max_tokens: int = 500,
        temperature: float = 0.7
    ) -> bool:
        """Test streaming request using requests library"""
        print_header("Testing Anthropic Streaming Request")

        if not HAS_REQUESTS:
            print_error("requests library not installed. Run: pip install requests")
            return False

        print(f"API URL: {self.api_url}")
        print(f"Model: {self.model}")
        print(f"Message: {message}")
        print(f"Max Tokens: {max_tokens}")
        print(f"Temperature: {temperature}")
        print("-" * 70)

        payload = {
            "model": self.model,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": [
                {"role": "user", "content": message}
            ],
            "stream": True
        }

        try:
            response = requests.post(
                self.api_url,
                headers=self.headers,
                json=payload,
                stream=True,
                timeout=120
            )

            print(f"\nStatus Code: {response.status_code}")

            if response.status_code != 200:
                print_error(f"HTTP Error: {response.status_code}")
                print(f"Response: {response.text}")
                return False

            print_success("Connection established")
            print("\nStreaming Response:")
            print("-" * 70)

            chunk_count = 0
            content_buffer = ""
            thinking_buffer = ""

            start_time = time.time()
            first_token_time = None

            for line in response.iter_lines():
                if line:
                    chunk_count += 1
                    decoded = line.decode('utf-8')

                    # Parse SSE format
                    if decoded.startswith("data: "):
                        data_str = decoded[6:]  # Remove "data: " prefix
                        try:
                            data = json.loads(data_str)
                            event_type = data.get("type", "")

                            # Handle different event types
                            if event_type == "message_start":
                                print_info("Message started")
                                if first_token_time is None:
                                    first_token_time = time.time()

                            elif event_type == "content_block_start":
                                print_info(f"Content block started: {data.get('content_block', {}).get('type', 'text')}")

                            elif event_type == "content_block_delta":
                                delta = data.get("delta", {})
                                if "text" in delta:
                                    text = delta["text"]
                                    content_buffer += text
                                    print(text, end='', flush=True)
                                if "thinking" in delta:
                                    thinking = delta["thinking"]
                                    thinking_buffer += thinking

                            elif event_type == "content_block_stop":
                                print("")  # New line after block

                            elif event_type == "message_delta":
                                delta = data.get("delta", {})
                                if "stop_reason" in delta:
                                    stop_reason = delta["stop_reason"]
                                    print_info(f"Stream ended. Reason: {stop_reason}")

                            elif event_type == "message_stop":
                                print_info("Message complete")

                            elif event_type == "ping":
                                print_color(Colors.DIM, "[ping]")

                            elif event_type == "error":
                                error = data.get("error", {})
                                print_error(f"API Error: {error.get('message', 'Unknown error')}")

                        except json.JSONDecodeError as e:
                            print(f"\n[JSON Parse Error]: {e}")
                            print(f"[Raw Data]: {data_str[:100]}")

            elapsed = time.time() - start_time
            time_to_first_token = (first_token_time - start_time) if first_token_time else 0

            print("-" * 70)
            print(f"\nStatistics:")
            print(f"  Total chunks: {chunk_count}")
            print(f"  Total time: {elapsed:.2f}s")
            print(f"  Time to first token: {time_to_first_token:.2f}s")
            print(f"  Characters received: {len(content_buffer)}")
            if thinking_buffer:
                print(f"  Thinking characters: {len(thinking_buffer)}")

            return True

        except requests.exceptions.Timeout:
            print_error("Request timed out")
            return False
        except requests.exceptions.ConnectionError as e:
            print_error(f"Connection error: {e}")
            return False
        except Exception as e:
            print_error(f"Unexpected error: {e}")
            import traceback
            traceback.print_exc()
            return False

    def test_non_streaming(
        self,
        message: str = "Say hello in one sentence.",
        max_tokens: int = 100
    ) -> bool:
        """Test non-streaming request for comparison"""
        print_header("Testing Non-Streaming Request")

        if not HAS_REQUESTS:
            print_error("requests library not installed")
            return False

        payload = {
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": [
                {"role": "user", "content": message}
            ]
        }

        try:
            start_time = time.time()
            response = requests.post(
                self.api_url,
                headers=self.headers,
                json=payload,
                timeout=60
            )
            elapsed = time.time() - start_time

            print(f"Status Code: {response.status_code}")

            if response.status_code != 200:
                print_error(f"HTTP Error: {response.status_code}")
                print(f"Response: {response.text}")
                return False

            data = response.json()

            print(f"\nResponse received in {elapsed:.2f}s:")
            print("-" * 70)

            if "content" in data:
                for block in data["content"]:
                    if block.get("type") == "text":
                        print(block["text"])
                    elif block.get("type") == "thinking":
                        print_color(Colors.DIM, f"[Thinking: {block['thinking'][:100]}...]")

            print("-" * 70)

            # Print usage info
            if "usage" in data:
                usage = data["usage"]
                print(f"\nToken Usage:")
                print(f"  Input tokens: {usage.get('input_tokens', 0)}")
                print(f"  Output tokens: {usage.get('output_tokens', 0)}")
                print(f"  Total tokens: {usage.get('input_tokens', 0) + usage.get('output_tokens', 0)}")

            return True

        except Exception as e:
            print_error(f"Error: {e}")
            return False

    def test_with_sdk(
        self,
        message: str = "Write a haiku about programming.",
        max_tokens: int = 500
    ) -> bool:
        """Test streaming using anthropic SDK"""
        print_header("Testing with Anthropic SDK")

        if not HAS_ANTHROPIC_SDK:
            print_info("Anthropic SDK not installed. Install with: pip install anthropic")
            return False

        try:
            client = anthropic.Anthropic(api_key=self.api_key)

            print(f"Model: {self.model}")
            print(f"Message: {message}")
            print("-" * 70)

            stream = client.messages.stream(
                model=self.model,
                max_tokens=max_tokens,
                messages=[{"role": "user", "content": message}]
            )

            print("\nStreaming Response:")
            print("-" * 70)

            with stream as s:
                for text in s.text_stream:
                    print(text, end="", flush=True)

            print("\n" + "-" * 70)

            message = stream.get_final_message()
            print_info(f"\nStop reason: {message.stop_reason}")
            print(f"Input tokens: {message.usage.input_tokens}")
            print(f"Output tokens: {message.usage.output_tokens}")

            return True

        except Exception as e:
            print_error(f"SDK Error: {e}")
            return False


def parse_args():
    """Parse command line arguments"""
    import argparse
    parser = argparse.ArgumentParser(
        description="Test Anthropic Claude API with streaming",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Basic streaming test (uses ANTHROPIC_API_KEY env var)
  python test_anthropic_stream.py

  # Test with custom API key
  python test_anthropic_stream.py --api-key sk-ant-xxx

  # Test with local proxy
  python test_anthropic_stream.py --api-url http://localhost:8787/v1/messages

  # Test specific model
  python test_anthropic_stream.py --model claude-3-opus-20240229

  # Custom message
  python test_anthropic_stream.py --message "Explain quantum computing"

  # Test non-streaming only
  python test_anthropic_stream.py --non-streaming-only

  # Test with SDK
  python test_anthropic_stream.py --use-sdk
        """
    )

    parser.add_argument(
        "--api-key", "-k",
        default=DEFAULT_API_KEY,
        help="Anthropic API key (default: from ANTHROPIC_API_KEY env var)"
    )
    parser.add_argument(
        "--api-url", "-u",
        default=DEFAULT_API_URL,
        help="API URL (default: Anthropic official API)"
    )
    parser.add_argument(
        "--model", "-m",
        default=DEFAULT_MODEL,
        help="Model to use (default: claude-3-5-sonnet-20241022)"
    )
    parser.add_argument(
        "--message",
        default="Hello! Please tell me a short joke.",
        help="Test message to send"
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=500,
        help="Maximum tokens in response"
    )
    parser.add_argument(
        "--temperature", "-t",
        type=float,
        default=0.7,
        help="Temperature (0.0-1.0)"
    )
    parser.add_argument(
        "--non-streaming-only",
        action="store_true",
        help="Only test non-streaming request"
    )
    parser.add_argument(
        "--streaming-only",
        action="store_true",
        help="Only test streaming request"
    )
    parser.add_argument(
        "--use-sdk",
        action="store_true",
        help="Use anthropic SDK instead of requests"
    )

    return parser.parse_args()


def main():
    """Main entry point"""
    args = parse_args()

    print_color(Colors.BOLD + Colors.HEADER, "Anthropic Claude Streaming Test")
    print("Testing Claude Messages API with streaming support")

    # Validate API key
    if not args.api_key:
        print_error("No API key provided!")
        print("\nPlease set ANTHROPIC_API_KEY environment variable or use --api-key")
        print("Get your API key from: https://console.anthropic.com/")
        return 1

    # Mask API key for display
    masked_key = f"{args.api_key[:7]}...{args.api_key[-4:]}" if len(args.api_key) > 11 else "***"
    print(f"API Key: {masked_key}")

    tester = AnthropicStreamTester(
        api_key=args.api_key,
        api_url=args.api_url,
        model=args.model
    )

    results = {}

    if args.use_sdk:
        success = tester.test_with_sdk(args.message, args.max_tokens)
        results["SDK Streaming"] = success
    else:
        if not args.non_streaming_only:
            results["Streaming"] = tester.test_streaming(
                message=args.message,
                max_tokens=args.max_tokens,
                temperature=args.temperature
            )

        if not args.streaming_only:
            results["Non-Streaming"] = tester.test_non_streaming(
                message="Say hello in one sentence.",
                max_tokens=100
            )

    # Print summary
    print_header("Test Summary")
    for test_name, success in results.items():
        status = "PASSED" if success else "FAILED"
        color = Colors.GREEN if success else Colors.RED
        print_color(color, f"{test_name}: {status}")

    all_passed = all(results.values())
    print("\n" + ("=" * 70))

    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
