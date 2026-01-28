use super::ModelAdapter;
use serde_json::Value;

#[allow(dead_code)]
pub struct OpenAIAdapter;

impl ModelAdapter for OpenAIAdapter {
    fn map_chat_request(&self, input: Value) -> Value {
        input
    }
    fn map_chat_response(&self, output: Value) -> Value {
        output
    }
}

#[allow(dead_code)]
impl OpenAIAdapter {
    pub fn id() -> &'static str {
        "openai"
    }
}
