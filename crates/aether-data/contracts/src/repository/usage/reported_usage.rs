use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Captured by the gateway usage builder, independently of client metadata and local estimates.
pub const REPORTED_USAGE_METADATA_KEY: &str = "reported_usage";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReportedUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_creation_ephemeral_5m_tokens: u64,
    pub cache_creation_ephemeral_1h_tokens: u64,
    pub cache_read_tokens: u64,
}

impl ReportedUsage {
    pub fn has_tokens(&self) -> bool {
        [
            self.input_tokens,
            self.output_tokens,
            self.cache_creation_tokens,
            self.cache_creation_ephemeral_5m_tokens,
            self.cache_creation_ephemeral_1h_tokens,
            self.cache_read_tokens,
        ]
        .into_iter()
        .any(|value| value > 0)
    }

    pub(super) fn is_valid(&self) -> bool {
        [
            self.input_tokens,
            self.output_tokens,
            self.cache_creation_tokens,
            self.cache_creation_ephemeral_5m_tokens,
            self.cache_creation_ephemeral_1h_tokens,
            self.cache_read_tokens,
        ]
        .into_iter()
        .all(|value| value <= i64::MAX as u64)
    }
}

pub fn reported_usage(metadata: Option<&Value>) -> Option<ReportedUsage> {
    let usage: ReportedUsage =
        serde_json::from_value(metadata?.get(REPORTED_USAGE_METADATA_KEY)?.clone()).ok()?;
    usage.is_valid().then_some(usage)
}

pub fn failed_usage_is_billable(metadata: Option<&Value>) -> bool {
    ![
        super::USAGE_AVAILABLE_METADATA_KEY,
        super::USAGE_PRICING_AVAILABLE_METADATA_KEY,
    ]
    .into_iter()
    .any(|key| {
        metadata
            .and_then(|metadata| metadata.get(key))
            .and_then(Value::as_bool)
            == Some(false)
    }) && reported_usage(metadata).is_some_and(|usage| usage.has_tokens())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_valid_reported_token_snapshot_can_make_failure_billable() {
        for value in [
            json!({}),
            json!({"input_tokens":-1}),
            json!({"output_tokens":1.5}),
            json!({"cache_read_tokens":u64::MAX}),
            json!({"image_count":1}),
        ] {
            assert!(!failed_usage_is_billable(Some(
                &json!({"reported_usage":value})
            )));
        }
        let mut metadata = json!({"reported_usage":{"cache_read_tokens":100}});
        assert!(failed_usage_is_billable(Some(&metadata)));
        for key in [
            super::super::USAGE_AVAILABLE_METADATA_KEY,
            super::super::USAGE_PRICING_AVAILABLE_METADATA_KEY,
        ] {
            metadata[key] = json!(false);
            assert!(!failed_usage_is_billable(Some(&metadata)));
            metadata.as_object_mut().unwrap().remove(key);
        }
        assert!(
            super::super::sanitize_usage_request_metadata(Some(metadata))
                .unwrap()
                .get(REPORTED_USAGE_METADATA_KEY)
                .is_some()
        );
    }
}
