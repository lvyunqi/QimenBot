//! Message representation and builder utilities for QimenBot.
//!
//! A [`Message`] is an ordered list of [`Segment`]s (text, images, at-mentions, etc.)
//! following the OneBot segment model. Use [`MessageBuilder`] for fluent construction.

pub mod cqcode;
pub mod keyboard;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A chat message composed of one or more [`Segment`]s.
///
/// Stores both the processed segments and optional raw representations
/// for round-tripping through different formats (CQ codes, OneBot JSON).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub raw_text: Option<String>,
    #[serde(default)]
    pub raw_segments: Option<Vec<Segment>>,
}

impl Message {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            raw_text: Some(text.clone()),
            segments: vec![Segment::text(text)],
            raw_segments: None,
        }
    }

    pub fn push(mut self, segment: Segment) -> Self {
        self.segments.push(segment);
        self
    }

    pub fn builder() -> MessageBuilder {
        MessageBuilder::default()
    }

    pub fn from_segments(segments: Vec<Segment>) -> Self {
        let raw_text = if segments.iter().all(|segment| segment.kind == "text") {
            Some(
                segments
                    .iter()
                    .filter_map(|segment| segment.data.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(""),
            )
        } else {
            None
        };

        Self {
            raw_text,
            raw_segments: Some(segments.clone()),
            segments,
        }
    }

    pub fn from_onebot_value(value: &Value) -> Self {
        match value {
            Value::String(text) => Self::from_cq_string(text),
            Value::Array(items) => {
                let segments = items
                    .iter()
                    .filter_map(Segment::from_onebot_value)
                    .collect::<Vec<_>>();
                Self {
                    raw_text: None,
                    raw_segments: Some(segments.clone()),
                    segments,
                }
            }
            Value::Object(_) => Segment::from_onebot_value(value)
                .map(|segment| Self {
                    raw_text: None,
                    raw_segments: Some(vec![segment.clone()]),
                    segments: vec![segment],
                })
                .unwrap_or_default(),
            _ => Self::default(),
        }
    }

    pub fn from_cq_string(input: &str) -> Self {
        let mut segments = Vec::new();
        let mut cursor = 0;

        while let Some(relative_start) = input[cursor..].find("[CQ:") {
            let start = cursor + relative_start;
            if start > cursor {
                segments.push(Segment::text(unescape_text(&input[cursor..start])));
            }

            let tail = &input[start..];
            if let Some(relative_end) = tail.find(']') {
                let end = start + relative_end + 1;
                let cq = &input[start..end];
                if let Some(segment) = Segment::from_cq_code(cq) {
                    segments.push(segment);
                } else {
                    segments.push(Segment::text(unescape_text(cq)));
                }
                cursor = end;
            } else {
                segments.push(Segment::text(unescape_text(tail)));
                cursor = input.len();
            }
        }

        if cursor < input.len() {
            segments.push(Segment::text(unescape_text(&input[cursor..])));
        }

        Self {
            raw_text: Some(input.to_string()),
            raw_segments: None,
            segments,
        }
    }

    pub fn to_onebot_value(&self) -> Value {
        if self.segments.is_empty() {
            return self
                .raw_text
                .clone()
                .map(Value::String)
                .unwrap_or_else(|| Value::Array(Vec::new()));
        }

        if self.segments.iter().all(|segment| segment.kind == "text") {
            return Value::String(self.plain_text());
        }

        Value::Array(self.segments.iter().map(Segment::to_onebot_value).collect())
    }

    pub fn plain_text(&self) -> String {
        self.segments
            .iter()
            .filter_map(|segment| {
                if segment.kind == "text" {
                    segment
                        .data
                        .get("text")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Check if message contains an @all segment
    pub fn has_at_all(&self) -> bool {
        self.segments.iter().any(|s| {
            s.kind == "at" && s.data.get("qq").and_then(|v| v.as_str()) == Some("all")
        })
    }

    /// Check if message contains an image segment
    pub fn has_image(&self) -> bool {
        self.segments.iter().any(|s| s.kind == "image")
    }

    /// Check if message contains a record (voice) segment
    pub fn has_record(&self) -> bool {
        self.segments.iter().any(|s| s.kind == "record")
    }

    /// Check if message contains a video segment
    pub fn has_video(&self) -> bool {
        self.segments.iter().any(|s| s.kind == "video")
    }

    /// Check if message contains a reply segment
    pub fn has_reply(&self) -> bool {
        self.segments.iter().any(|s| s.kind == "reply")
    }

    /// Get the reply message_id if present
    pub fn reply_id(&self) -> Option<&str> {
        self.segments
            .iter()
            .find(|s| s.kind == "reply")
            .and_then(|s| s.data.get("id"))
            .and_then(|v| v.as_str())
    }

    /// All @-mention targets (user IDs), excluding `"all"`.
    pub fn at_list(&self) -> Vec<&str> {
        self.segments
            .iter()
            .filter(|s| s.kind == "at")
            .filter_map(|s| s.data.get("qq").and_then(|v| v.as_str()))
            .filter(|qq| *qq != "all")
            .collect()
    }

    /// Whether the message @-mentions a specific user ID.
    pub fn has_at(&self, user_id: &str) -> bool {
        self.segments.iter().any(|s| {
            s.kind == "at" && s.data.get("qq").and_then(|v| v.as_str()) == Some(user_id)
        })
    }

    /// All image URLs in the message (prefers `data["url"]`, falls back to `data["file"]`).
    pub fn image_urls(&self) -> Vec<&str> {
        self.segments
            .iter()
            .filter(|s| s.kind == "image")
            .filter_map(|s| {
                s.data
                    .get("url")
                    .and_then(|v| v.as_str())
                    .or_else(|| s.data.get("file").and_then(|v| v.as_str()))
            })
            .collect()
    }

    /// All voice/record URLs in the message.
    pub fn record_urls(&self) -> Vec<&str> {
        self.segments
            .iter()
            .filter(|s| s.kind == "record")
            .filter_map(|s| {
                s.data
                    .get("url")
                    .and_then(|v| v.as_str())
                    .or_else(|| s.data.get("file").and_then(|v| v.as_str()))
            })
            .collect()
    }

    /// All video URLs in the message.
    pub fn video_urls(&self) -> Vec<&str> {
        self.segments
            .iter()
            .filter(|s| s.kind == "video")
            .filter_map(|s| {
                s.data
                    .get("url")
                    .and_then(|v| v.as_str())
                    .or_else(|| s.data.get("file").and_then(|v| v.as_str()))
            })
            .collect()
    }
}

/// Fluent builder for constructing [`Message`] instances segment by segment.
///
/// ```ignore
/// let msg = Message::builder()
///     .text("Hello ")
///     .at("123456")
///     .image("https://example.com/pic.png")
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
pub struct MessageBuilder {
    segments: Vec<Segment>,
}

impl MessageBuilder {
    /// Append a plain text segment.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.segments.push(Segment::text(text));
        self
    }

    /// Append an @-mention segment. Use `"all"` to @everyone.
    pub fn at(mut self, target: impl Into<String>) -> Self {
        self.segments.push(Segment::at(target));
        self
    }

    /// Append a reply-quote segment referencing another message.
    pub fn reply(mut self, message_id: impl Into<String>) -> Self {
        self.segments.push(Segment::reply(message_id));
        self
    }

    /// Append an image segment (URL or local path).
    pub fn image(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::image(file));
        self
    }

    /// Append a QQ face/emoji segment by ID.
    pub fn face(mut self, id: impl Into<String>) -> Self {
        self.segments.push(Segment::face(id));
        self
    }

    pub fn record(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::record(file));
        self
    }

    pub fn video(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::video(file));
        self
    }

    pub fn rps(mut self) -> Self {
        self.segments.push(Segment::rps());
        self
    }

    pub fn dice(mut self) -> Self {
        self.segments.push(Segment::dice());
        self
    }

    pub fn shake(mut self) -> Self {
        self.segments.push(Segment::shake());
        self
    }

    pub fn poke(mut self, poke_type: impl Into<String>, id: impl Into<String>) -> Self {
        self.segments.push(Segment::poke(poke_type, id));
        self
    }

    pub fn anonymous(mut self) -> Self {
        self.segments.push(Segment::anonymous());
        self
    }

    pub fn share(mut self, url: impl Into<String>, title: impl Into<String>) -> Self {
        self.segments.push(Segment::share(url, title));
        self
    }

    pub fn contact(mut self, contact_type: impl Into<String>, id: impl Into<String>) -> Self {
        self.segments.push(Segment::contact(contact_type, id));
        self
    }

    pub fn location(mut self, lat: f64, lon: f64, title: impl Into<String>) -> Self {
        self.segments.push(Segment::location(lat, lon, title));
        self
    }

    pub fn music(mut self, music_type: impl Into<String>, id: impl Into<String>) -> Self {
        self.segments.push(Segment::music(music_type, id));
        self
    }

    pub fn music_custom(
        mut self,
        url: impl Into<String>,
        audio: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        self.segments.push(Segment::music_custom(url, audio, title));
        self
    }

    pub fn forward(mut self, id: impl Into<String>) -> Self {
        self.segments.push(Segment::forward(id));
        self
    }

    pub fn node(
        mut self,
        user_id: impl Into<String>,
        nickname: impl Into<String>,
        content: Message,
    ) -> Self {
        self.segments.push(Segment::node(user_id, nickname, content));
        self
    }

    pub fn xml(mut self, data: impl Into<String>) -> Self {
        self.segments.push(Segment::xml(data));
        self
    }

    pub fn json_msg(mut self, data: impl Into<String>) -> Self {
        self.segments.push(Segment::json_msg(data));
        self
    }

    /// Append an arbitrary pre-built segment.
    pub fn segment(mut self, segment: Segment) -> Self {
        self.segments.push(segment);
        self
    }

    /// TTS (text-to-speech) message
    pub fn tts(mut self, text: impl Into<String>) -> Self {
        self.segments.push(Segment::tts(text));
        self
    }

    /// Card image (big image in group chat, Go-CQHTTP extension)
    pub fn card_image(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::card_image(file));
        self
    }

    /// Markdown message (NapCat/Lagrange extension)
    pub fn markdown(mut self, content: impl Into<String>) -> Self {
        self.segments.push(Segment::markdown(content));
        self
    }

    /// @all shorthand
    pub fn at_all(mut self) -> Self {
        self.segments.push(Segment::at("all"));
        self
    }

    /// Flash image (disappearing image)
    pub fn flash_image(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::flash_image(file));
        self
    }

    /// Image with custom attributes (cache, proxy)
    pub fn image_with_opts(mut self, file: impl Into<String>, cache: bool, proxy: bool) -> Self {
        self.segments.push(Segment::image_with_opts(file, cache, proxy));
        self
    }

    /// Record with magic voice effect
    pub fn record_magic(mut self, file: impl Into<String>) -> Self {
        self.segments.push(Segment::record_magic(file));
        self
    }

    /// Append an interactive keyboard segment.
    pub fn keyboard(mut self, kb: crate::keyboard::Keyboard) -> Self {
        self.segments.push(kb.to_segment());
        self
    }

    /// Consume the builder and produce the final [`Message`].
    pub fn build(self) -> Message {
        Message::from_segments(self.segments)
    }
}

/// A single message segment (e.g. text, image, at-mention, face, reply).
///
/// `kind` holds the segment type string (`"text"`, `"image"`, `"at"`, etc.)
/// and `data` holds the type-specific key-value parameters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub data: Map<String, Value>,
}

impl Segment {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            data: Map::new(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        let mut segment = Self::new("text");
        segment
            .data
            .insert("text".to_string(), Value::String(text.into()));
        segment
    }

    pub fn at(target: impl Into<String>) -> Self {
        let mut segment = Self::new("at");
        segment
            .data
            .insert("qq".to_string(), Value::String(target.into()));
        segment
    }

    pub fn reply(message_id: impl Into<String>) -> Self {
        let mut segment = Self::new("reply");
        segment
            .data
            .insert("id".to_string(), Value::String(message_id.into()));
        segment
    }

    pub fn image(file: impl Into<String>) -> Self {
        let mut segment = Self::new("image");
        segment
            .data
            .insert("file".to_string(), Value::String(file.into()));
        segment
    }

    pub fn face(id: impl Into<String>) -> Self {
        let mut segment = Self::new("face");
        segment
            .data
            .insert("id".to_string(), Value::String(id.into()));
        segment
    }

    pub fn record(file: impl Into<String>) -> Self {
        let mut segment = Self::new("record");
        segment
            .data
            .insert("file".to_string(), Value::String(file.into()));
        segment
    }

    pub fn video(file: impl Into<String>) -> Self {
        let mut segment = Self::new("video");
        segment
            .data
            .insert("file".to_string(), Value::String(file.into()));
        segment
    }

    pub fn rps() -> Self {
        Self::new("rps")
    }

    pub fn dice() -> Self {
        Self::new("dice")
    }

    pub fn shake() -> Self {
        Self::new("shake")
    }

    pub fn poke(poke_type: impl Into<String>, id: impl Into<String>) -> Self {
        let mut segment = Self::new("poke");
        segment
            .data
            .insert("type".to_string(), Value::String(poke_type.into()));
        segment
            .data
            .insert("id".to_string(), Value::String(id.into()));
        segment
    }

    pub fn anonymous() -> Self {
        Self::new("anonymous")
    }

    pub fn share(url: impl Into<String>, title: impl Into<String>) -> Self {
        let mut segment = Self::new("share");
        segment
            .data
            .insert("url".to_string(), Value::String(url.into()));
        segment
            .data
            .insert("title".to_string(), Value::String(title.into()));
        segment
    }

    pub fn contact(contact_type: impl Into<String>, id: impl Into<String>) -> Self {
        let mut segment = Self::new("contact");
        segment
            .data
            .insert("type".to_string(), Value::String(contact_type.into()));
        segment
            .data
            .insert("id".to_string(), Value::String(id.into()));
        segment
    }

    pub fn location(lat: f64, lon: f64, title: impl Into<String>) -> Self {
        let mut segment = Self::new("location");
        segment
            .data
            .insert("lat".to_string(), Value::String(lat.to_string()));
        segment
            .data
            .insert("lon".to_string(), Value::String(lon.to_string()));
        segment
            .data
            .insert("title".to_string(), Value::String(title.into()));
        segment
    }

    pub fn music(music_type: impl Into<String>, id: impl Into<String>) -> Self {
        let mut segment = Self::new("music");
        segment
            .data
            .insert("type".to_string(), Value::String(music_type.into()));
        segment
            .data
            .insert("id".to_string(), Value::String(id.into()));
        segment
    }

    pub fn music_custom(
        url: impl Into<String>,
        audio: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        let mut segment = Self::new("music");
        segment
            .data
            .insert("type".to_string(), Value::String("custom".to_string()));
        segment
            .data
            .insert("url".to_string(), Value::String(url.into()));
        segment
            .data
            .insert("audio".to_string(), Value::String(audio.into()));
        segment
            .data
            .insert("title".to_string(), Value::String(title.into()));
        segment
    }

    pub fn forward(id: impl Into<String>) -> Self {
        let mut segment = Self::new("forward");
        segment
            .data
            .insert("id".to_string(), Value::String(id.into()));
        segment
    }

    pub fn node(
        user_id: impl Into<String>,
        nickname: impl Into<String>,
        content: Message,
    ) -> Self {
        let mut segment = Self::new("node");
        segment
            .data
            .insert("user_id".to_string(), Value::String(user_id.into()));
        segment
            .data
            .insert("nickname".to_string(), Value::String(nickname.into()));
        segment
            .data
            .insert("content".to_string(), content.to_onebot_value());
        segment
    }

    pub fn xml(data: impl Into<String>) -> Self {
        let mut segment = Self::new("xml");
        segment
            .data
            .insert("data".to_string(), Value::String(data.into()));
        segment
    }

    pub fn json_msg(data: impl Into<String>) -> Self {
        let mut segment = Self::new("json");
        segment
            .data
            .insert("data".to_string(), Value::String(data.into()));
        segment
    }

    pub fn tts(text: impl Into<String>) -> Self {
        let mut segment = Self::new("tts");
        segment
            .data
            .insert("text".to_string(), Value::String(text.into()));
        segment
    }

    pub fn card_image(file: impl Into<String>) -> Self {
        let mut segment = Self::new("cardimage");
        segment
            .data
            .insert("file".to_string(), Value::String(file.into()));
        segment
    }

    pub fn markdown(content: impl Into<String>) -> Self {
        let mut segment = Self::new("markdown");
        segment
            .data
            .insert("content".to_string(), Value::String(content.into()));
        segment
    }

    pub fn flash_image(file: impl Into<String>) -> Self {
        let mut segment = Self::new("image");
        segment
            .data
            .insert("file".to_string(), Value::String(file.into()));
        segment
            .data
            .insert("type".to_string(), Value::String("flash".to_string()));
        segment
    }

    pub fn image_with_opts(file: impl Into<String>, cache: bool, proxy: bool) -> Self {
        let mut segment = Self::new("image");
        segment
            .data
            .insert("file".to_string(), Value::String(file.into()));
        segment.data.insert(
            "cache".to_string(),
            Value::String(if cache { "1" } else { "0" }.to_string()),
        );
        segment.data.insert(
            "proxy".to_string(),
            Value::String(if proxy { "1" } else { "0" }.to_string()),
        );
        segment
    }

    pub fn record_magic(file: impl Into<String>) -> Self {
        let mut segment = Self::new("record");
        segment
            .data
            .insert("file".to_string(), Value::String(file.into()));
        segment
            .data
            .insert("magic".to_string(), Value::String("1".to_string()));
        segment
    }

    pub fn with(mut self, key: impl Into<String>, value: Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    // ── Type checks ──

    /// Whether this is a text segment.
    pub fn is_text(&self) -> bool {
        self.kind == "text"
    }

    /// Whether this is an @-mention segment.
    pub fn is_at(&self) -> bool {
        self.kind == "at"
    }

    // ── Data accessors ──

    /// Get a `data` field as `&str`.
    pub fn data_str(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_str())
    }

    /// Get a `data` field as a lossless `String` (handles both string and number JSON values).
    pub fn data_lossless(&self, key: &str) -> Option<String> {
        self.data.get(key).map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            other => other.to_string(),
        })
    }

    /// Extract the text content from a text segment.
    pub fn get_text(&self) -> Option<&str> {
        if self.kind == "text" {
            self.data_str("text")
        } else {
            None
        }
    }

    /// Extract the @-mention target (handles both `"qq"` and `"id"` keys).
    /// Returns a lossless string since the value may be a JSON number or string.
    pub fn at_target(&self) -> Option<String> {
        if self.kind != "at" {
            return None;
        }
        self.data_lossless("qq")
            .or_else(|| self.data_lossless("id"))
    }

    pub fn from_onebot_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?.to_string();
        let data = object
            .get("data")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        Some(Self { kind, data })
    }

    pub fn from_cq_code(input: &str) -> Option<Self> {
        if !input.starts_with("[CQ:") || !input.ends_with(']') {
            return None;
        }

        let inner = &input[4..input.len() - 1];
        let mut parts = inner.split(',');
        let kind = parts.next()?.trim();
        if kind.is_empty() {
            return None;
        }

        let mut segment = Self::new(kind.to_string());
        for part in parts {
            let (key, raw_value) = part.split_once('=')?;
            segment
                .data
                .insert(key.to_string(), Value::String(unescape_cq_value(raw_value)));
        }
        Some(segment)
    }

    pub fn to_onebot_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("type".to_string(), Value::String(self.kind.clone()));
        object.insert("data".to_string(), Value::Object(self.data.clone()));
        Value::Object(object)
    }
}

fn unescape_text(input: &str) -> String {
    input
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&amp;", "&")
}

fn unescape_cq_value(input: &str) -> String {
    input
        .replace("&#44;", ",")
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_at_list() {
        let msg = Message::builder()
            .text("hello ")
            .at("111")
            .at("all")
            .at("222")
            .build();
        assert_eq!(msg.at_list(), vec!["111", "222"]);
    }

    #[test]
    fn test_has_at() {
        let msg = Message::builder().at("123").text(" hi").build();
        assert!(msg.has_at("123"));
        assert!(!msg.has_at("999"));
    }

    #[test]
    fn test_image_urls_with_url_field() {
        let seg = Segment::new("image")
            .with("url", Value::String("https://img.example.com/a.png".into()))
            .with("file", Value::String("file:///local.png".into()));
        let msg = Message::from_segments(vec![seg]);
        // prefers url over file
        assert_eq!(msg.image_urls(), vec!["https://img.example.com/a.png"]);
    }

    #[test]
    fn test_image_urls_fallback_to_file() {
        let seg = Segment::image("https://example.com/b.png");
        let msg = Message::from_segments(vec![seg]);
        assert_eq!(msg.image_urls(), vec!["https://example.com/b.png"]);
    }

    #[test]
    fn test_record_urls() {
        let seg = Segment::record("https://example.com/voice.amr");
        let msg = Message::from_segments(vec![seg]);
        assert_eq!(msg.record_urls(), vec!["https://example.com/voice.amr"]);
    }

    #[test]
    fn test_video_urls() {
        let seg = Segment::video("https://example.com/video.mp4");
        let msg = Message::from_segments(vec![seg]);
        assert_eq!(msg.video_urls(), vec!["https://example.com/video.mp4"]);
    }

    #[test]
    fn test_empty_at_list() {
        let msg = Message::text("no mentions");
        assert!(msg.at_list().is_empty());
    }

    // ── Segment convenience method tests ──

    #[test]
    fn test_segment_is_text() {
        assert!(Segment::text("hi").is_text());
        assert!(!Segment::at("123").is_text());
    }

    #[test]
    fn test_segment_is_at() {
        assert!(Segment::at("123").is_at());
        assert!(!Segment::text("hi").is_at());
    }

    #[test]
    fn test_segment_get_text() {
        assert_eq!(Segment::text("hello").get_text(), Some("hello"));
        assert_eq!(Segment::at("123").get_text(), None);
    }

    #[test]
    fn test_segment_at_target() {
        assert_eq!(Segment::at("12345").at_target(), Some("12345".to_string()));
        assert_eq!(Segment::text("hi").at_target(), None);
    }

    #[test]
    fn test_segment_at_target_numeric_value() {
        let seg = Segment::new("at").with("qq", Value::Number(12345.into()));
        assert_eq!(seg.at_target(), Some("12345".to_string()));
    }

    #[test]
    fn test_segment_data_str() {
        let seg = Segment::text("foo");
        assert_eq!(seg.data_str("text"), Some("foo"));
        assert_eq!(seg.data_str("nonexistent"), None);
    }

    #[test]
    fn test_segment_data_lossless() {
        let seg = Segment::new("test")
            .with("str_field", Value::String("abc".into()))
            .with("num_field", Value::Number(42.into()));
        assert_eq!(seg.data_lossless("str_field"), Some("abc".to_string()));
        assert_eq!(seg.data_lossless("num_field"), Some("42".to_string()));
        assert_eq!(seg.data_lossless("missing"), None);
    }
}
