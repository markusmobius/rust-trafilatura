use serde::Deserialize;
use serde_json::Value;
use std::fmt::Write;

#[path = "json_input.rs"]
mod input;

pub(crate) struct DecodedJson(pub Value);

impl Drop for DecodedJson {
    fn drop(&mut self) {
        let mut pending = vec![std::mem::take(&mut self.0)];
        while let Some(value) = pending.pop() {
            match value {
                Value::Array(children) => pending.extend(children),
                Value::Object(children) => pending.extend(children.into_values()),
                _ => {}
            }
        }
    }
}

pub(crate) fn parse(source: &str) -> Option<DecodedJson> {
    let source = input::prepare(source)?;
    let mut decoder = serde_json::Deserializer::from_str(&source);
    decoder.disable_recursion_limit();
    let decoded =
        DecodedJson(Value::deserialize(serde_stacker::Deserializer::new(&mut decoder)).ok()?);
    decoder.end().ok()?;
    Some(decoded)
}

pub(crate) fn decode(source: &str) -> Option<DecodedJson> {
    if let Some(decoded) = parse(source) {
        return Some(decoded);
    }
    let mut escaped = String::with_capacity(source.len());
    let mut quoted = false;
    let mut backslash = false;
    let mut changed = false;
    for character in source.chars() {
        if quoted && !backslash && character < '\u{20}' {
            write!(escaped, "\\u00{:02x}", character as u32).unwrap();
            changed = true;
        } else {
            escaped.push(character);
        }
        if backslash {
            backslash = false;
        } else if quoted && character == '\\' {
            backslash = true;
        } else if character == '"' {
            quoted = !quoted;
        }
    }
    if changed {
        parse(&escaped)
    } else {
        None
    }
}

pub(crate) fn items(value: &Value) -> &[Value] {
    match value {
        Value::Null => &[],
        Value::Array(items) => items,
        other => std::slice::from_ref(other),
    }
}
