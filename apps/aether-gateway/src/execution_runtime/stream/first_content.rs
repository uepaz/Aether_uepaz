use serde_json::Value;
use std::sync::Arc;

use crate::execution_runtime::StreamCandidateWatchdogProgress;

use super::commit_policy::find_sse_record_boundary;

/// 首字按完整协议事件中的实际输出判定，不把网络分块或保活当作生成进展。
pub(super) struct FirstContentObserver {
    sse: bool,
    buffered: Vec<u8>,
    observed: bool,
    oversized: bool,
    watchdog_progress: Option<Arc<StreamCandidateWatchdogProgress>>,
}

impl FirstContentObserver {
    pub(super) fn new(sse: bool) -> Self {
        Self {
            sse,
            buffered: Vec::new(),
            observed: false,
            oversized: false,
            // task-local 不跨转发任务传播，进度句柄必须随观察器一起交接。
            watchdog_progress: StreamCandidateWatchdogProgress::current(),
        }
    }

    pub(super) fn observe(&mut self, chunk: &[u8]) -> bool {
        if self.observed || chunk.is_empty() || self.oversized {
            return self.observed;
        }
        if !self.sse {
            self.mark_observed();
            return true;
        }
        // 只限制用于首字判断的副本；超大或未结束的事件不能凭字节量被认作内容。
        if self.buffered.len().saturating_add(chunk.len())
            > crate::execution_runtime::MAX_STREAM_BODY_CAPTURE_BYTES
        {
            self.buffered = Vec::new();
            self.oversized = true;
            return false;
        }
        self.buffered.extend_from_slice(chunk);
        let mut consumed = 0;
        while let Some((end, separator)) = find_sse_record_boundary(&self.buffered[consumed..]) {
            let record = &self.buffered[consumed..consumed + end];
            if record_has_content(record) {
                self.mark_observed();
                self.buffered = Vec::new();
                return true;
            }
            consumed += end + separator;
        }
        if consumed > 0 {
            self.buffered.drain(..consumed);
        }
        false
    }

    fn mark_observed(&mut self) {
        self.observed = true;
        if let Some(progress) = self.watchdog_progress.as_ref() {
            progress.mark_first_content_observed();
        }
    }
}

fn record_has_content(record: &[u8]) -> bool {
    let Ok(record) = std::str::from_utf8(record) else {
        return false;
    };
    let normalized = record.replace("\r\n", "\n").replace('\r', "\n");
    let event = normalized
        .lines()
        .find_map(|line| line.strip_prefix("event:").map(str::trim));
    if event.is_some_and(is_control_event) {
        return false;
    }
    let data = normalized
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
        .collect::<Vec<_>>()
        .join("\n");
    let Ok(payload) = serde_json::from_str::<Value>(&data) else {
        return false;
    };
    payload_has_content(&payload, event)
}

fn is_control_event(kind: &str) -> bool {
    matches!(
        kind,
        "ping"
            | "heartbeat"
            | "keepalive"
            | "error"
            | "response.created"
            | "response.in_progress"
            | "response.queued"
            | "response.failed"
            | "response.incomplete"
    )
}

fn nonempty(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

pub(super) fn payload_has_content(payload: &Value, event: Option<&str>) -> bool {
    let kind = payload
        .get("type")
        .and_then(Value::as_str)
        .or(event)
        .unwrap_or_default();
    if is_control_event(kind) || payload.get("error").is_some_and(|error| !error.is_null()) {
        return false;
    }
    if let Some(choices) = payload.get("choices").and_then(Value::as_array) {
        return choices.iter().any(|choice| {
            nonempty(choice.get("text"))
                || choice
                    .get("delta")
                    .or_else(|| choice.get("message"))
                    .is_some_and(chat_delta_has_content)
        });
    }
    if let Some(response) = payload.get("response") {
        if payload_has_content(response, None) {
            return true;
        }
    }
    if let Some(candidates) = payload.get("candidates").and_then(Value::as_array) {
        return candidates
            .iter()
            .filter_map(|candidate| {
                candidate
                    .pointer("/content/parts")
                    .and_then(Value::as_array)
            })
            .flatten()
            .any(|part| {
                nonempty(part.get("text"))
                    || nonempty(part.pointer("/functionCall/name"))
                    || nonempty(part.pointer("/functionResponse/name"))
                    || nonempty(part.pointer("/inlineData/data"))
                    || nonempty(part.pointer("/fileData/fileUri"))
                    || nonempty(part.pointer("/executableCode/code"))
                    || nonempty(part.pointer("/codeExecutionResult/output"))
            });
    }
    match kind {
        "message_start" => payload
            .pointer("/message/content")
            .and_then(Value::as_array)
            .is_some_and(|blocks| blocks.iter().any(block_has_content)),
        "content_block_start" => payload.get("content_block").is_some_and(block_has_content),
        "content_block_delta" => payload.get("delta").is_some_and(|delta| {
            nonempty(delta.get("text"))
                || nonempty(delta.get("thinking"))
                || nonempty(delta.get("partial_json"))
        }),
        "response.output_text.delta"
        | "response.refusal.delta"
        | "response.reasoning_text.delta"
        | "response.reasoning_summary_text.delta"
        | "response.function_call_arguments.delta"
        | "response.custom_tool_call_input.delta"
        | "response.output_audio.delta"
        | "response.audio.delta"
        | "response.output_audio_transcript.delta"
        | "response.audio_transcript.delta" => nonempty(payload.get("delta")),
        "response.output_item.added" | "response.output_item.done" => {
            payload.get("item").is_some_and(block_has_content)
        }
        "response.content_part.added"
        | "response.content_part.done"
        | "response.reasoning_summary_part.added"
        | "response.reasoning_summary_part.done" => {
            payload.get("part").is_some_and(block_has_content)
        }
        "response.image_generation_call.partial_image"
        | "image_generation.partial_image"
        | "image_generation.completed" => {
            nonempty(payload.get("partial_image_b64")) || nonempty(payload.get("b64_json"))
        }
        "response.completed" => payload
            .pointer("/response/output")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(block_has_content)),
        kind if kind.starts_with("response.") && kind.ends_with(".delta") => {
            nonempty(payload.get("delta"))
        }
        kind if kind.starts_with("response.") && kind.ends_with(".done") => {
            ["text", "arguments", "input", "refusal", "transcript"]
                .iter()
                .any(|key| nonempty(payload.get(key)))
        }
        _ => false,
    }
}

fn chat_delta_has_content(delta: &Value) -> bool {
    [
        "content",
        "reasoning_content",
        "reasoning",
        "thinking",
        "refusal",
    ]
    .iter()
    .any(|key| nonempty(delta.get(key)))
        || delta
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|parts| parts.iter().any(block_has_content))
        || delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| {
                calls
                    .iter()
                    .any(|call| call.get("function").is_some_and(function_has_content))
            })
        || delta.get("function_call").is_some_and(function_has_content)
        || nonempty(delta.pointer("/audio/data"))
        || nonempty(delta.pointer("/audio/transcript"))
}

fn function_has_content(function: &Value) -> bool {
    nonempty(function.get("name")) || nonempty(function.get("arguments"))
}

fn block_has_content(block: &Value) -> bool {
    match block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "text" | "output_text" | "summary_text" => nonempty(block.get("text")),
        "thinking" => nonempty(block.get("thinking")),
        "redacted_thinking" => nonempty(block.get("data")),
        "refusal" => nonempty(block.get("refusal")),
        "tool_use" | "server_tool_use" | "function_call" | "custom_tool_call" => {
            nonempty(block.get("name"))
        }
        "reasoning" => {
            nonempty(block.get("encrypted_content"))
                || block
                    .get("summary")
                    .and_then(Value::as_array)
                    .is_some_and(|parts| parts.iter().any(block_has_content))
        }
        "message" => block
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|parts| parts.iter().any(block_has_content)),
        "image_generation_call" => nonempty(block.get("result")),
        "audio" | "output_audio" => nonempty(block.get("data")),
        "image" | "image_url" | "output_image" => {
            nonempty(block.get("url"))
                || nonempty(block.get("image_url"))
                || nonempty(block.pointer("/image_url/url"))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::FirstContentObserver;

    #[tokio::test]
    async fn first_content_progress_survives_forwarding_task_handoff() {
        let progress = crate::execution_runtime::StreamCandidateWatchdogProgress::shared();
        progress
            .clone()
            .scope(async {
                let mut observer = FirstContentObserver::new(true);
                let progress_for_task = progress.clone();
                tokio::spawn(async move {
                    assert!(!observer.observe(b": keepalive\n\n"));
                    assert!(!progress_for_task.first_content_observed());
                    assert!(observer
                        .observe(b"data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n"));
                })
                .await
                .unwrap();
            })
            .await;
        assert!(progress.first_content_observed());
    }

    #[test]
    fn heartbeats_setup_empty_deltas_errors_and_terminal_markers_are_not_first_content() {
        let mut observer = FirstContentObserver::new(true);
        for chunk in [
            ": keepalive\r\n\r\n", "event: ping\ndata: {\"type\":\"ping\"}\n\n",
            "data: {\"type\":\"heartbeat\"}\n\n", "data: {\"type\":\"response.created\"}\n\n",
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0}]}}]}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"\"}\n\n",
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"signature_delta\",\"signature\":\"abc\"}}\n\n",
            "data: {\"error\":{\"message\":\"busy\"}}\n\n", "data: [DONE]\n\n",
        ] { assert!(!observer.observe(chunk.as_bytes()), "{chunk}"); }
    }

    #[test]
    fn fragmented_text_thinking_and_tools_establish_first_content_across_protocols() {
        for chunk in [
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"thinking\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"function\":{\"name\":\"search\"}}]}}]}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{}\"}\n\n",
            "data: {\"type\":\"response.output_text.done\",\"text\":\"hello\"}\n\n",
            "data: {\"type\":\"image_generation.completed\",\"b64_json\":\"aGk=\"}\n\n",
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"thinking\"}}\n\n",
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hello\"}]}}]}\n\n",
            "data: {\"response\":{\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"search\"}}]}}]}}\r\n\r\n",
        ] {
            for split in 1..chunk.len() - 1 {
                let mut observer = FirstContentObserver::new(true);
                observer.observe(&chunk.as_bytes()[..split]);
                assert!(observer.observe(&chunk.as_bytes()[split..]), "split={split}: {chunk}");
            }
        }
    }
}
