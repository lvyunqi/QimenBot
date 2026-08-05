use async_trait::async_trait;
use qimen_error::Result;
use qimen_message::{Message, Segment};
use qimen_plugin_api::Module;
use qimen_protocol_core::NormalizedEvent;

#[derive(Default)]
pub struct CommandModule;

#[async_trait]
impl Module for CommandModule {
    fn id(&self) -> &'static str {
        "command"
    }

    async fn on_load(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandTrigger {
    Private,
    Group,
    Prefix,
    Mention,
    Reply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedCommandInput {
    pub trigger: CommandTrigger,
    pub command_text: String,
    pub source_text: String,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandTriggerPolicy<'a> {
    pub prefixes: &'a [String],
    pub private_bare_enabled: bool,
    pub group_bare_enabled: bool,
    pub mention_enabled: bool,
    pub reply_enabled: bool,
}

pub fn match_command_input(
    event: &NormalizedEvent,
    policy: CommandTriggerPolicy<'_>,
) -> Option<MatchedCommandInput> {
    let message = event.message.as_ref()?;

    // Structural triggers must be checked before plain-text prefixes. A reply
    // segment can still render as `/command` in `plain_text()`, which would
    // otherwise make it look like an ordinary prefixed message.
    if policy.mention_enabled
        && let Some(command_text) = strip_mention_command_text(event, message, policy.prefixes)
    {
        return Some(MatchedCommandInput {
            trigger: CommandTrigger::Mention,
            command_text,
            source_text: message.plain_text(),
        });
    }

    if policy.reply_enabled
        && let Some(command_text) = strip_reply_command_text(message, policy.prefixes)
    {
        return Some(MatchedCommandInput {
            trigger: CommandTrigger::Reply,
            command_text,
            source_text: message.plain_text(),
        });
    }

    if let Some(command_text) = strip_prefixed_command_text(message, policy.prefixes) {
        return Some(MatchedCommandInput {
            trigger: CommandTrigger::Prefix,
            command_text,
            source_text: message.plain_text(),
        });
    }

    if event.is_group() && policy.group_bare_enabled {
        let source_text = message.plain_text();
        let command_text = source_text.trim().to_string();
        if !command_text.is_empty() {
            return Some(MatchedCommandInput {
                trigger: CommandTrigger::Group,
                command_text,
                source_text,
            });
        }
    }

    if event.is_private() && policy.private_bare_enabled {
        let source_text = message.plain_text();
        let command_text = source_text.trim().to_string();
        if !command_text.is_empty() {
            return Some(MatchedCommandInput {
                trigger: CommandTrigger::Private,
                command_text,
                source_text,
            });
        }
    }

    None
}

pub fn strip_command_name_and_args(input: &str) -> Option<(&str, Vec<String>)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let name = parts.next()?;
    let args = parts.map(|part| part.to_string()).collect();
    Some((name, args))
}

fn strip_prefixed_command_text(message: &Message, prefixes: &[String]) -> Option<String> {
    let text = message.plain_text();
    strip_configured_prefix(text.trim(), prefixes).map(str::to_string)
}

fn strip_mention_command_text(
    event: &NormalizedEvent,
    message: &Message,
    prefixes: &[String],
) -> Option<String> {
    if !event.is_at_self() {
        return None;
    }
    let self_id = event.self_id_str();
    let remaining = strip_leading_self_mentions(&message.segments, self_id.as_deref())?;
    let text = remaining
        .iter()
        .filter_map(segment_text)
        .collect::<Vec<_>>()
        .join("");
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(strip_optional_prefix(trimmed, prefixes).to_string())
}

fn strip_reply_command_text(message: &Message, prefixes: &[String]) -> Option<String> {
    let remaining = strip_leading_reply_segments(&message.segments)?;
    let text = remaining
        .iter()
        .filter_map(segment_text)
        .collect::<Vec<_>>()
        .join("");
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(strip_optional_prefix(trimmed, prefixes).to_string())
    }
}

fn strip_configured_prefix<'a>(input: &'a str, prefixes: &[String]) -> Option<&'a str> {
    prefixes
        .iter()
        .filter(|prefix| input.starts_with(prefix.as_str()))
        .max_by_key(|prefix| prefix.len())
        .map(|prefix| input[prefix.len()..].trim())
        .filter(|command| !command.is_empty())
}

fn strip_optional_prefix<'a>(input: &'a str, prefixes: &[String]) -> &'a str {
    strip_configured_prefix(input, prefixes).unwrap_or(input)
}

fn strip_leading_self_mentions<'a>(
    segments: &'a [Segment],
    self_id: Option<&str>,
) -> Option<&'a [Segment]> {
    let mut index = 0;
    let mut seen_mention = false;

    while let Some(segment) = segments.get(index) {
        if is_self_at_segment(segment, self_id) {
            seen_mention = true;
            index += 1;
            continue;
        }

        if seen_mention && is_whitespace_text_segment(segment) {
            index += 1;
            continue;
        }

        break;
    }

    if seen_mention {
        Some(&segments[index..])
    } else {
        None
    }
}

fn strip_leading_reply_segments(segments: &[Segment]) -> Option<&[Segment]> {
    let mut index = 0;
    let mut seen_reply = false;

    while let Some(segment) = segments.get(index) {
        if segment.kind == "reply" {
            seen_reply = true;
            index += 1;
            continue;
        }

        if seen_reply && is_whitespace_text_segment(segment) {
            index += 1;
            continue;
        }

        break;
    }

    if seen_reply {
        Some(&segments[index..])
    } else {
        None
    }
}

fn is_self_at_segment(segment: &Segment, self_id: Option<&str>) -> bool {
    if segment
        .data
        .get("is_self")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        return true;
    }
    let Some(self_id) = self_id else {
        return false;
    };
    segment.at_target().is_some_and(|target| target == self_id)
}

fn is_whitespace_text_segment(segment: &Segment) -> bool {
    segment
        .get_text()
        .is_some_and(|text| text.trim().is_empty())
}

fn segment_text(segment: &Segment) -> Option<&str> {
    segment.get_text()
}

#[cfg(test)]
mod tests {
    use super::{CommandTrigger, CommandTriggerPolicy, match_command_input};
    use qimen_message::{Message, Segment};
    use qimen_protocol_core::{ChatRef, EventKind, NormalizedEvent, ProtocolId, TransportMode};
    use serde_json::{Map, json};

    fn event(chat_kind: &str, message: Message) -> NormalizedEvent {
        NormalizedEvent {
            protocol: ProtocolId::OneBot11,
            bot_instance: "test-bot".to_string(),
            transport_mode: TransportMode::WsForward,
            time: Some(1),
            kind: EventKind::Message,
            message: Some(message),
            actor: None,
            chat: Some(ChatRef {
                id: "chat-id".to_string(),
                kind: chat_kind.to_string(),
            }),
            raw_json: json!({"self_id": 10001}),
            raw_bytes: None,
            extensions: Map::new(),
        }
    }

    fn policy<'a>(prefixes: &'a [String]) -> CommandTriggerPolicy<'a> {
        CommandTriggerPolicy {
            prefixes,
            private_bare_enabled: true,
            group_bare_enabled: true,
            mention_enabled: true,
            reply_enabled: true,
        }
    }

    #[test]
    fn configured_prefix_uses_longest_match() {
        let prefixes = vec!["/".to_string(), "//".to_string()];
        let matched = match_command_input(
            &event("group", Message::text("//deploy now")),
            policy(&prefixes),
        )
        .unwrap();
        assert_eq!(matched.trigger, CommandTrigger::Prefix);
        assert_eq!(matched.command_text, "deploy now");
    }

    #[test]
    fn private_bare_command_is_optional() {
        let prefixes = vec!["/".to_string()];
        let private = event("private", Message::text("status"));
        let matched = match_command_input(&private, policy(&prefixes)).unwrap();
        assert_eq!(matched.trigger, CommandTrigger::Private);
        assert_eq!(matched.command_text, "status");

        let disabled = CommandTriggerPolicy {
            private_bare_enabled: false,
            ..policy(&prefixes)
        };
        assert!(match_command_input(&private, disabled).is_none());
    }

    #[test]
    fn group_bare_command_is_available_when_enabled() {
        let prefixes = vec!["/".to_string()];
        let matched =
            match_command_input(&event("group", Message::text("status")), policy(&prefixes));
        let matched = matched.unwrap();
        assert_eq!(matched.trigger, CommandTrigger::Group);
        assert_eq!(matched.command_text, "status");

        let disabled = CommandTriggerPolicy {
            group_bare_enabled: false,
            ..policy(&prefixes)
        };
        assert!(match_command_input(&event("group", Message::text("status")), disabled).is_none());
    }

    #[test]
    fn mention_and_reply_are_independent_triggers() {
        let prefixes = vec!["/".to_string()];
        let mention = Segment::at("10001").with("is_self", json!(true));
        let mentioned = event(
            "group",
            Message::from_segments(vec![mention, Segment::text(" help 2")]),
        );
        let matched = match_command_input(&mentioned, policy(&prefixes)).unwrap();
        assert_eq!(matched.trigger, CommandTrigger::Mention);
        assert_eq!(matched.command_text, "help 2");

        let replied = event(
            "group",
            Message::from_segments(vec![
                Segment::new("reply").with("id", json!("message-id")),
                Segment::text(" /help 3"),
            ]),
        );
        let matched = match_command_input(&replied, policy(&prefixes)).unwrap();
        assert_eq!(matched.trigger, CommandTrigger::Reply);
        assert_eq!(matched.command_text, "help 3");
    }
}
