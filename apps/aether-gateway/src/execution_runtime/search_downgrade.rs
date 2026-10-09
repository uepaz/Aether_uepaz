use aether_contracts::ExecutionPlan;
use serde_json::{json, Value};

pub(super) fn has_chat_search(plan: &ExecutionPlan) -> bool {
    plan.provider_api_format.eq_ignore_ascii_case("openai:chat")
        && plan.body.body_bytes_b64.is_none()
        && plan.body.json_body.as_ref().is_some_and(|body| {
            body.get("web_search_options")
                .is_some_and(|options| !options.is_null())
        })
}

pub(super) fn downgrade_unsupported_chat_search(
    plan: &mut ExecutionPlan,
    status_code: u16,
    response_text: Option<&str>,
) -> Option<Value> {
    // Only explicit request/capability rejections permit dropping search. In
    // particular, authentication, overload and parameter-value errors do not.
    if !has_chat_search(plan) || !matches!(status_code, 400 | 422) {
        return None;
    }
    let text = response_text?;
    let parsed = serde_json::from_str::<Value>(text).ok();
    let error = parsed
        .as_ref()
        .and_then(|body| body.get("error"))
        .or(parsed.as_ref());
    let param = error
        .and_then(|error| error.get("param"))
        .and_then(Value::as_str);
    let search_param = param.is_some_and(|param| {
        param == "web_search_options" || param.starts_with("web_search_options.")
    });
    if param.is_some_and(|param| !param.is_empty()) && !search_param {
        return None;
    }
    let code = error
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let message = error
        .and_then(|error| {
            error
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| error.as_str())
        })
        .unwrap_or(if parsed.is_none() { text } else { "" })
        .to_ascii_lowercase();
    let explicitly_rejected = [
        "not supported",
        "unsupported",
        "does not support",
        "doesn't support",
        "unknown parameter",
        "unrecognized request argument",
        "unrecognized parameter",
        "unexpected keyword argument",
        "extra inputs are not permitted",
    ]
    .iter()
    .any(|term| message.contains(term));
    // Bind the rejection to search, rather than matching an unsupported
    // function or generation parameter mentioned elsewhere in the error.
    let normalized_message = message
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let search_rejection = ["web_search_options", "web_search", "web search"]
        .iter()
        .any(|term| {
            [
                format!("{term} not supported"),
                format!("{term} is not supported"),
                format!("{term} is unsupported"),
                format!("unsupported {term}"),
                format!("does not support {term}"),
                format!("does not support the {term}"),
                format!("doesn t support {term}"),
                format!("unsupported parameter {term}"),
                format!("unknown parameter {term}"),
                format!("unrecognized parameter {term}"),
                format!("unrecognized request argument supplied {term}"),
                format!("unexpected keyword argument {term}"),
            ]
            .iter()
            .any(|pattern| normalized_message.contains(pattern))
        });
    if !(search_param && matches!(code, "unsupported_parameter" | "unsupported_feature"))
        && !(search_param && explicitly_rejected)
        && !search_rejection
    {
        return None;
    }
    plan.body
        .json_body
        .as_mut()?
        .as_object_mut()?
        .remove("web_search_options")?;
    plan.headers
        .retain(|name, _| !name.eq_ignore_ascii_case("content-length"));
    let original_error =
        parsed.unwrap_or_else(|| Value::String(text.chars().take(16_384).collect()));
    Some(json!({
        "search_downgraded": true,
        "search_downgrade_reason": "upstream_search_unsupported",
        "search_downgrade_original_error": {"status_code":status_code, "body":original_error},
        "search_downgrade_policy": "retry_same_candidate_without_search_once"
    }))
}

pub(super) fn attach_search_downgrade_context(context: &mut Option<Value>, record: &Value) {
    let context = context.get_or_insert_with(|| json!({}));
    if let (Some(context), Some(record)) = (context.as_object_mut(), record.as_object()) {
        context.extend(record.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_contracts::RequestBody;
    use serde_json::json;

    fn plan() -> ExecutionPlan {
        serde_json::from_value(json!({
            "request_id":"search-test", "candidate_id":"candidate", "provider_id":"provider",
            "endpoint_id":"endpoint", "key_id":"key", "method":"POST",
            "url":"https://example.test/v1/chat/completions", "headers":{},
            "body":{}, "stream":true, "client_api_format":"openai:responses",
            "provider_api_format":"openai:chat"
        }))
        .unwrap()
    }

    #[test]
    fn search_downgrade_preserves_request_and_is_one_shot() {
        let mut plan = plan();
        let original = json!({
            "model":"deepseek-v4.1-flash", "messages":[{"role":"user","content":"hello"}],
            "web_search_options":{"search_context_size":"medium"},
            "tools":[{"type":"function","function":{"name":"web_search","parameters":{"type":"object"}}}],
            "tool_choice":"auto", "stream":true, "temperature":0.2
        });
        plan.body = RequestBody::from_json(original.clone());
        let error = r#"{"error":{"code":"unsupported_parameter","param":"web_search_options","message":"This model does not support web search"}}"#;
        let record = downgrade_unsupported_chat_search(&mut plan, 400, Some(error)).unwrap();
        let mut expected = original;
        expected
            .as_object_mut()
            .unwrap()
            .remove("web_search_options");
        assert_eq!(plan.body.json_body, Some(expected));
        assert_eq!(record["search_downgraded"], true);
        assert_eq!(
            record["search_downgrade_original_error"]["status_code"],
            400
        );
        assert!(downgrade_unsupported_chat_search(&mut plan, 400, Some(error)).is_none());
    }

    #[test]
    fn search_downgrade_rejects_unrelated_errors() {
        for (status, error) in [
            (401, "web_search_options not supported"),
            (403, "web_search_options not supported"),
            (429, "web_search_options not supported"),
            (500, "web_search_options not supported"),
            (
                400,
                r#"{"error":{"param":"web_search_options.search_context_size","message":"Invalid value: huge"}}"#,
            ),
            (
                400,
                r#"{"error":{"param":"tools","message":"Unsupported function schema"}}"#,
            ),
            (
                400,
                r#"{"error":{"param":"temperature","code":"unsupported_parameter","message":"web_search_options is available; temperature is not supported"}}"#,
            ),
            (400, "Unsupported function schema for web_search"),
            (
                400,
                "web_search_options is available; temperature is not supported",
            ),
        ] {
            let mut plan = plan();
            plan.body = RequestBody::from_json(json!({"web_search_options":{}}));
            let before = plan.clone();
            assert!(
                downgrade_unsupported_chat_search(&mut plan, status, Some(error)).is_none(),
                "{status} {error}"
            );
            assert_eq!(plan, before);
        }
    }

    #[test]
    fn search_downgrade_accepts_explicit_compatibility_errors() {
        for error in [
            r#"{"error":{"message":"Unrecognized request argument supplied: web_search_options"}}"#,
            r#"{"error":{"message":"This model does not support web search"}}"#,
            r#"{"error":{"param":"web_search_options","code":"unsupported_parameter"}}"#,
            "Unknown parameter: web_search_options",
        ] {
            let mut plan = plan();
            plan.body = RequestBody::from_json(json!({"web_search_options":{}}));
            assert!(
                downgrade_unsupported_chat_search(&mut plan, 400, Some(error)).is_some(),
                "{error}"
            );
        }
    }
}
