//! Integration tests for CQ code parsing and serialization roundtrips.

use qimen_message::Message;
use qimen_message::cqcode::{parse_cq_string, to_cq_string};

#[test]
fn complex_cq_string_roundtrip() {
    let input = "[CQ:at,qq=123456]你好[CQ:face,id=178]世界[CQ:image,file=abc.jpg]";
    let message = parse_cq_string(input);
    assert_eq!(message.segments.len(), 5); // at, text, face, text, image

    assert_eq!(message.segments[0].kind, "at");
    assert_eq!(message.segments[1].kind, "text");
    assert_eq!(
        message.segments[1].data.get("text").and_then(serde_json::Value::as_str),
        Some("你好")
    );
    assert_eq!(message.segments[2].kind, "face");
    assert_eq!(message.segments[3].kind, "text");
    assert_eq!(
        message.segments[3].data.get("text").and_then(serde_json::Value::as_str),
        Some("世界")
    );
    assert_eq!(message.segments[4].kind, "image");

    let output = to_cq_string(&message);
    assert_eq!(output, input);
}

#[test]
fn empty_message_roundtrip() {
    let input = "";
    let message = parse_cq_string(input);
    assert!(message.segments.is_empty());
    assert_eq!(to_cq_string(&message), "");
}

#[test]
fn plain_text_only() {
    let input = "just plain text with no CQ codes";
    let message = parse_cq_string(input);
    assert_eq!(message.segments.len(), 1);
    assert_eq!(message.plain_text(), input);
}

#[test]
fn consecutive_cq_codes_no_text_between() {
    let input = "[CQ:at,qq=111][CQ:at,qq=222][CQ:face,id=1]";
    let message = parse_cq_string(input);
    assert_eq!(message.segments.len(), 3);
    assert_eq!(message.segments[0].kind, "at");
    assert_eq!(message.segments[1].kind, "at");
    assert_eq!(message.segments[2].kind, "face");
    assert_eq!(to_cq_string(&message), input);
}

#[test]
fn cq_code_with_special_chars_in_value() {
    // Build a message with a value containing commas, brackets, and ampersands
    let msg = Message::from_segments(vec![
        qimen_message::Segment::new("test")
            .with("data".to_string(), serde_json::Value::String("a,b[c]&d".to_string())),
    ]);
    let cq = to_cq_string(&msg);
    // Values should be escaped
    assert!(cq.contains("&#44;")); // comma
    assert!(cq.contains("&#91;")); // [
    assert!(cq.contains("&#93;")); // ]
    assert!(cq.contains("&amp;")); // &

    // Roundtrip should recover original value
    let parsed = parse_cq_string(&cq);
    assert_eq!(parsed.segments.len(), 1);
    assert_eq!(
        parsed.segments[0]
            .data
            .get("data")
            .and_then(serde_json::Value::as_str),
        Some("a,b[c]&d")
    );
}

#[test]
fn text_with_special_chars_roundtrip() {
    let msg = Message::from_segments(vec![qimen_message::Segment::text("hello [world] & friends")]);
    let cq = to_cq_string(&msg);
    let parsed = parse_cq_string(&cq);
    assert_eq!(parsed.plain_text(), "hello [world] & friends");
}

#[test]
fn mixed_text_and_multiple_segment_types() {
    let input = "前缀[CQ:reply,id=12345]引用[CQ:at,qq=67890] 你好[CQ:image,file=test.png]后缀";
    let message = parse_cq_string(input);
    // Expected: text, reply, text, at, text, image, text
    assert_eq!(message.segments.len(), 7);
    assert_eq!(message.segments[0].kind, "text");
    assert_eq!(message.segments[1].kind, "reply");
    assert_eq!(message.segments[2].kind, "text");
    assert_eq!(message.segments[3].kind, "at");
    assert_eq!(message.segments[4].kind, "text");
    assert_eq!(message.segments[5].kind, "image");
    assert_eq!(message.segments[6].kind, "text");

    let output = to_cq_string(&message);
    assert_eq!(output, input);
}
