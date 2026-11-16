use serde_json::{Map, Value};

/// 由实际响应协议选择字段，避免兼容别名覆盖原生用量（包括明确的零值）。
#[derive(Clone, Copy)]
pub enum OpenAiUsageFormat {
    Chat,
    Responses,
}

impl OpenAiUsageFormat {
    pub fn input_tokens(self, usage: &Map<String, Value>) -> Option<&Value> {
        self.field(usage, "prompt_tokens", "input_tokens")
    }

    pub fn output_tokens(self, usage: &Map<String, Value>) -> Option<&Value> {
        self.field(usage, "completion_tokens", "output_tokens")
    }

    pub fn cache_read_tokens(self, usage: &Map<String, Value>) -> Option<&Value> {
        self.input_detail(usage, "cached_tokens")
            .or_else(|| usage.get("cache_read_input_tokens"))
    }

    pub fn cache_write_tokens(self, usage: &Map<String, Value>) -> Option<&Value> {
        self.input_detail(usage, "cache_write_tokens")
            .or_else(|| self.input_detail(usage, "cached_creation_tokens"))
            .or_else(|| self.input_detail(usage, "cache_creation_tokens"))
            .or_else(|| usage.get("cache_creation_input_tokens"))
    }

    pub fn reasoning_tokens(self, usage: &Map<String, Value>) -> Option<&Value> {
        self.detail(
            usage,
            "completion_tokens_details",
            "output_tokens_details",
            "reasoning_tokens",
        )
        .or_else(|| usage.get("reasoning_tokens"))
    }

    fn field<'a>(
        self,
        usage: &'a Map<String, Value>,
        chat: &str,
        responses: &str,
    ) -> Option<&'a Value> {
        let (primary, alias) = self.keys(chat, responses);
        usage.get(primary).or_else(|| usage.get(alias))
    }

    fn input_detail<'a>(self, usage: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
        self.detail(usage, "prompt_tokens_details", "input_tokens_details", key)
    }

    fn detail<'a>(
        self,
        usage: &'a Map<String, Value>,
        chat: &str,
        responses: &str,
        key: &str,
    ) -> Option<&'a Value> {
        let (primary, alias) = self.keys(chat, responses);
        usage
            .get(primary)
            .and_then(|details| details.get(key))
            .or_else(|| usage.get(alias).and_then(|details| details.get(key)))
    }

    fn keys<'a>(self, chat: &'a str, responses: &'a str) -> (&'a str, &'a str) {
        match self {
            Self::Chat => (chat, responses),
            Self::Responses => (responses, chat),
        }
    }
}
