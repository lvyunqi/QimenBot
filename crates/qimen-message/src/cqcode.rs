//! CQ code format parsing and generation.
//!
//! Provides [`parse_cq_string`] to convert CQ-coded strings into [`Message`]
//! instances and [`to_cq_string`] to serialize messages back to the CQ format.

use crate::{Message, Segment};
use serde_json::Value;

/// Escape special CQ characters in text: & -> &amp;  [ -> &#91;  ] -> &#93;
pub fn cq_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
}

/// Unescape CQ special characters
pub fn cq_unescape(text: &str) -> String {
    text.replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&amp;", "&")
}

/// Escape special characters inside CQ code values: & -> &amp;  [ -> &#91;  ] -> &#93;  , -> &#44;
fn escape_cq_value(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace(',', "&#44;")
}

/// Unescape characters inside CQ code values
fn unescape_cq_value(text: &str) -> String {
    text.replace("&#44;", ",")
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&amp;", "&")
}

/// Convert a CQ-coded string to a Message.
///
/// Example: `"[CQ:at,qq=123456]hello[CQ:face,id=178]"`
pub fn parse_cq_string(input: &str) -> Message {
    let mut segments = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = input[cursor..].find("[CQ:") {
        let start = cursor + relative_start;
        if start > cursor {
            segments.push(Segment::text(cq_unescape(&input[cursor..start])));
        }

        let tail = &input[start..];
        if let Some(relative_end) = tail.find(']') {
            let end = start + relative_end + 1;
            let cq = &input[start..end];
            if let Some(segment) = parse_cq_code(cq) {
                segments.push(segment);
            } else {
                segments.push(Segment::text(cq_unescape(cq)));
            }
            cursor = end;
        } else {
            segments.push(Segment::text(cq_unescape(tail)));
            cursor = input.len();
        }
    }

    if cursor < input.len() {
        segments.push(Segment::text(cq_unescape(&input[cursor..])));
    }

    Message::from_segments(segments)
}

/// Parse a single CQ code string like `[CQ:at,qq=123456]` into a Segment.
fn parse_cq_code(input: &str) -> Option<Segment> {
    if !input.starts_with("[CQ:") || !input.ends_with(']') {
        return None;
    }

    let inner = &input[4..input.len() - 1];
    let mut parts = inner.split(',');
    let kind = parts.next()?.trim();
    if kind.is_empty() {
        return None;
    }

    let mut segment = Segment::new(kind.to_string());
    for part in parts {
        let (key, raw_value) = part.split_once('=')?;
        segment
            .data
            .insert(key.to_string(), Value::String(unescape_cq_value(raw_value)));
    }
    Some(segment)
}

/// Convert a Message to CQ-coded string.
pub fn to_cq_string(message: &Message) -> String {
    let mut out = String::new();
    for segment in &message.segments {
        if segment.kind == "text" {
            if let Some(text) = segment.data.get("text").and_then(Value::as_str) {
                out.push_str(&cq_escape(text));
            }
        } else {
            out.push_str("[CQ:");
            out.push_str(&segment.kind);
            for (key, value) in &segment.data {
                out.push(',');
                out.push_str(key);
                out.push('=');
                if let Some(s) = value.as_str() {
                    out.push_str(&escape_cq_value(s));
                } else {
                    out.push_str(&escape_cq_value(&value.to_string()));
                }
            }
            out.push(']');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_text_only() {
        let input = "hello world";
        let msg = parse_cq_string(input);
        assert_eq!(msg.segments.len(), 1);
        assert_eq!(msg.segments[0].kind, "text");
        let result = to_cq_string(&msg);
        assert_eq!(result, input);
    }

    #[test]
    fn roundtrip_with_at() {
        let input = "[CQ:at,qq=123456]hello[CQ:face,id=178]";
        let msg = parse_cq_string(input);
        assert_eq!(msg.segments.len(), 3);
        assert_eq!(msg.segments[0].kind, "at");
        assert_eq!(
            msg.segments[0].data.get("qq").and_then(Value::as_str),
            Some("123456")
        );
        assert_eq!(msg.segments[1].kind, "text");
        assert_eq!(msg.segments[2].kind, "face");
        assert_eq!(
            msg.segments[2].data.get("id").and_then(Value::as_str),
            Some("178")
        );

        let result = to_cq_string(&msg);
        assert_eq!(result, input);
    }

    #[test]
    fn escape_special_chars() {
        assert_eq!(cq_escape("a&b[c]d"), "a&amp;b&#91;c&#93;d");
        assert_eq!(cq_unescape("a&amp;b&#91;c&#93;d"), "a&b[c]d");
    }

    #[test]
    fn roundtrip_with_special_chars_in_text() {
        let msg = Message::from_segments(vec![Segment::text("hello [world] & friends")]);
        let cq = to_cq_string(&msg);
        assert_eq!(cq, "hello &#91;world&#93; &amp; friends");
        let parsed = parse_cq_string(&cq);
        assert_eq!(parsed.plain_text(), "hello [world] & friends");
    }

    #[test]
    fn value_with_comma() {
        // Values containing commas should be escaped in CQ output
        let mut seg = Segment::new("test");
        seg.data.insert(
            "data".to_string(),
            serde_json::Value::String("a,b".to_string()),
        );
        let msg = Message::from_segments(vec![seg]);
        let cq = to_cq_string(&msg);
        assert_eq!(cq, "[CQ:test,data=a&#44;b]");
        let parsed = parse_cq_string(&cq);
        assert_eq!(parsed.segments.len(), 1);
        assert_eq!(
            parsed.segments[0]
                .data
                .get("data")
                .and_then(Value::as_str),
            Some("a,b")
        );
    }
}
