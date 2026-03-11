//! Runtime evaluation of [`MessageFilter`] rules against incoming events.

use qimen_plugin_api::{AtMode, MatchResult, MediaType, MessageFilter, ReplyFilter};
use qimen_protocol_core::NormalizedEvent;
use regex::Regex;

/// Evaluate a [`MessageFilter`] against a [`NormalizedEvent`], returning whether
/// the filter matched and any regex capture groups.
pub fn filter_matches(filter: &MessageFilter, event: &NormalizedEvent) -> MatchResult {
    let mut captures = Vec::new();
    let plain_text = event
        .message
        .as_ref()
        .map(|m| m.plain_text())
        .unwrap_or_default();

    if let Some(cmd_pattern) = &filter.cmd {
        match Regex::new(cmd_pattern) {
            Ok(re) => {
                if let Some(caps) = re.captures(&plain_text) {
                    for i in 0..caps.len() {
                        if let Some(m) = caps.get(i) {
                            captures.push(m.as_str().to_string());
                        }
                    }
                } else {
                    return finalize(filter, false, captures);
                }
            }
            Err(_) => {
                return finalize(filter, false, captures);
            }
        }
    }

    if let Some(prefix) = &filter.starts_with {
        if !plain_text.starts_with(prefix.as_str()) {
            return finalize(filter, false, captures);
        }
    }

    if let Some(suffix) = &filter.ends_with {
        if !plain_text.ends_with(suffix.as_str()) {
            return finalize(filter, false, captures);
        }
    }

    if let Some(substr) = &filter.contains {
        if !plain_text.contains(substr.as_str()) {
            return finalize(filter, false, captures);
        }
    }

    if !filter.groups.is_empty() {
        let group_id = event.group_id_i64().unwrap_or(0);
        if !filter.groups.contains(&group_id) {
            return finalize(filter, false, captures);
        }
    }

    if !filter.senders.is_empty() {
        let sender_id = event.sender_id_i64().unwrap_or(0);
        if !filter.senders.contains(&sender_id) {
            return finalize(filter, false, captures);
        }
    }

    // at_mode check
    match &filter.at_mode {
        AtMode::Both => {} // no check
        AtMode::Need | AtMode::NotNeed => {
            let has_at_bot = event.is_at_self();
            let require_at = filter.at_mode == AtMode::Need;
            if require_at != has_at_bot {
                return finalize(filter, false, captures);
            }
        }
    }

    // reply_filter check
    match &filter.reply_filter {
        ReplyFilter::None => {} // no check
        ReplyFilter::ReplyMe => {
            let has_reply = event
                .message
                .as_ref()
                .is_some_and(|m| m.has_reply());
            if !has_reply {
                return finalize(filter, false, captures);
            }
        }
        ReplyFilter::ReplyOther => {
            let self_id = event.self_id_str().unwrap_or_default();
            let has_reply_to_other = event
                .message
                .as_ref()
                .map(|m| {
                    m.segments.iter().any(|seg| {
                        if seg.kind != "reply" {
                            return false;
                        }
                        // If the reply segment has a user_id field, check it's not the bot
                        seg.data_lossless("user_id")
                            .map(|uid| uid != self_id)
                            .unwrap_or(true) // no user_id means we can't confirm it's the bot
                    })
                })
                .unwrap_or(false);
            if !has_reply_to_other {
                return finalize(filter, false, captures);
            }
        }
    }

    // media_types check
    if !filter.media_types.is_empty() {
        let has_media = event
            .message
            .as_ref()
            .map(|m| {
                m.segments.iter().any(|seg| {
                    filter.media_types.iter().any(|mt| {
                        let expected = match mt {
                            MediaType::Image => "image",
                            MediaType::Record => "record",
                            MediaType::Video => "video",
                        };
                        seg.kind == expected
                    })
                })
            })
            .unwrap_or(false);
        if !has_media {
            return finalize(filter, false, captures);
        }
    }

    finalize(filter, true, captures)
}

fn finalize(filter: &MessageFilter, matched: bool, captures: Vec<String>) -> MatchResult {
    MatchResult {
        matched: if filter.invert { !matched } else { matched },
        captures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qimen_message::Message;
    use qimen_protocol_core::{EventKind, ProtocolId, TransportMode};
    use serde_json::Map;

    fn sample_event(text: &str) -> NormalizedEvent {
        NormalizedEvent {
            protocol: ProtocolId::OneBot11,
            bot_instance: "test".to_string(),
            transport_mode: TransportMode::WsForward,
            time: Some(1),
            kind: EventKind::Message,
            message: Some(Message::text(text)),
            actor: Some(qimen_protocol_core::ActorRef {
                id: "10001".to_string(),
                display_name: None,
            }),
            chat: Some(qimen_protocol_core::ChatRef {
                id: "20001".to_string(),
                kind: "group".to_string(),
            }),
            raw_json: serde_json::json!({
                "self_id": 123456,
                "group_id": 20001,
                "user_id": 10001,
            }),
            raw_bytes: None,
            extensions: Map::new(),
        }
    }

    #[test]
    fn empty_filter_matches_everything() {
        let filter = MessageFilter::default();
        let event = sample_event("hello");
        assert!(filter_matches(&filter, &event).matched);
    }

    #[test]
    fn cmd_regex_matches() {
        let filter = MessageFilter {
            cmd: Some(r"^hello (\w+)$".to_string()),
            ..Default::default()
        };
        let event = sample_event("hello world");
        let result = filter_matches(&filter, &event);
        assert!(result.matched);
        assert_eq!(result.captures.len(), 2);
        assert_eq!(result.captures[1], "world");
    }

    #[test]
    fn cmd_regex_no_match() {
        let filter = MessageFilter {
            cmd: Some(r"^goodbye".to_string()),
            ..Default::default()
        };
        let event = sample_event("hello world");
        assert!(!filter_matches(&filter, &event).matched);
    }

    #[test]
    fn group_whitelist_filters() {
        let filter = MessageFilter {
            groups: vec![99999],
            ..Default::default()
        };
        let event = sample_event("hello");
        assert!(!filter_matches(&filter, &event).matched);
    }

    #[test]
    fn sender_whitelist_filters() {
        let filter = MessageFilter {
            senders: vec![10001],
            ..Default::default()
        };
        let event = sample_event("hello");
        assert!(filter_matches(&filter, &event).matched);
    }

    #[test]
    fn invert_reverses_result() {
        let filter = MessageFilter {
            invert: true,
            ..Default::default()
        };
        let event = sample_event("hello");
        assert!(!filter_matches(&filter, &event).matched);
    }
}
