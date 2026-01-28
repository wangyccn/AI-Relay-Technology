use super::ModelAdapter;
use serde_json::Value;

#[allow(dead_code)]
pub struct GeminiAdapter;

impl ModelAdapter for GeminiAdapter {
    fn map_chat_request(&self, input: Value) -> Value {
        input
    }
    fn map_chat_response(&self, output: Value) -> Value {
        output
    }
}

#[allow(dead_code)]
impl GeminiAdapter {
    pub fn id() -> &'static str {
        "gemini"
    }
}
