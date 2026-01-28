use super::ModelAdapter;
use serde_json::Value;

#[allow(dead_code)]
pub struct AnthropicAdapter;

impl ModelAdapter for AnthropicAdapter {
    fn map_chat_request(&self, input: Value) -> Value {
        input
    }
    fn map_chat_response(&self, output: Value) -> Value {
        output
    }
}

#[allow(dead_code)]
impl AnthropicAdapter {
    pub fn id() -> &'static str {
        "anthropic"
    }
}
