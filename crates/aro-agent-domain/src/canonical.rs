use serde::Serialize;
use serde_json::Value;

use crate::{AgentDomainError, ContentDigest};

/// Serializes JSON with recursively sorted object keys and no insignificant whitespace.
///
/// Arrays retain their order. Numbers use serde_json's normalized representation. Callers must
/// model unordered collections as BTreeMap/BTreeSet or sort them before hashing.
pub fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, AgentDomainError> {
    let value = serde_json::to_value(value)
        .map_err(|error| AgentDomainError::Canonicalization(error.to_string()))?;
    let mut output = Vec::new();
    write_value(&value, &mut output)?;
    Ok(output)
}

pub fn canonical_json_digest<T: Serialize>(value: &T) -> Result<ContentDigest, AgentDomainError> {
    canonical_json_bytes(value).map(|bytes| ContentDigest::from_bytes(&bytes))
}

fn write_value(value: &Value, output: &mut Vec<u8>) -> Result<(), AgentDomainError> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => {
            output.extend_from_slice(if *value { b"true" } else { b"false" });
        }
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => {
            let encoded = serde_json::to_string(value)
                .map_err(|error| AgentDomainError::Canonicalization(error.to_string()))?;
            output.extend_from_slice(encoded.as_bytes());
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_value(item, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_unstable_by_key(|(key, _)| *key);
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                let encoded_key = serde_json::to_string(key)
                    .map_err(|error| AgentDomainError::Canonicalization(error.to_string()))?;
                output.extend_from_slice(encoded_key.as_bytes());
                output.push(b':');
                write_value(item, output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}
