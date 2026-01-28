pub mod anthropic;
pub mod gemini;
pub mod openai;

use serde_json::Value;

// 这些适配器为将来的请求/响应转换功能保留
#[allow(dead_code)]
pub trait ModelAdapter {
    fn map_chat_request(&self, input: Value) -> Value;
    fn map_chat_response(&self, output: Value) -> Value;
}
