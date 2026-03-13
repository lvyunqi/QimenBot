//! Plugin API for QimenBot — traits, types, and helpers for building bot plugins.
//!
//! This crate defines the core abstractions that plugin developers implement:
//! [`CommandPlugin`] for text-command handlers, [`SystemPlugin`] for notice/request/meta
//! event handlers, and [`Module`] for grouping plugins into loadable units.

pub use inventory;
use async_trait::async_trait;
use qimen_error::Result;
use qimen_message::Message;
use qimen_protocol_core::{
    ActionMeta, CapabilitySet, NormalizedActionRequest, NormalizedActionResponse, NormalizedEvent,
    ProtocolId,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::sync::Arc;

/// A boxed, `Send + 'static` future that returns `()`. Used for spawning
/// background tasks via [`RuntimeBotContext::spawn_owned`].
pub type OwnedTaskFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>;

/// Handle returned when a background task is spawned on the runtime.
#[derive(Debug, Clone)]
pub struct TaskHandle {
    pub name: String,
}

/// Runtime context provided to plugins during event processing.
///
/// Gives plugins access to the current bot identity, protocol capabilities,
/// and the ability to send actions or spawn background tasks.
#[async_trait]
pub trait RuntimeBotContext: Send + Sync {
    /// Returns the bot instance identifier (e.g. `"bot-main"`).
    fn bot_instance(&self) -> &str;
    /// Returns the protocol this bot is connected with.
    fn protocol(&self) -> ProtocolId;
    /// Returns the capability set advertised by the protocol adapter.
    fn capabilities(&self) -> &CapabilitySet;

    /// Send a normalized action request and await the response.
    async fn send_action(&self, req: NormalizedActionRequest) -> Result<NormalizedActionResponse>;
    /// Convenience method: reply to an event with a message.
    async fn reply(&self, event: &NormalizedEvent, message: Message)
        -> Result<NormalizedActionResponse>;
    /// Spawn a long-running background task on the runtime's task pool.
    fn spawn_owned(&self, name: &str, fut: OwnedTaskFuture) -> TaskHandle;
}

/// High-level client for invoking OneBot 11 API actions.
///
/// Wraps a [`RuntimeBotContext`] reference and provides typed methods for
/// common operations such as sending messages, querying group info, and
/// managing group members. Create one via [`OneBotActionClient::new`] or
/// [`CommandPluginContext::onebot_actions`].
pub struct OneBotActionClient<'a> {
    ctx: &'a dyn RuntimeBotContext,
}

impl<'a> Clone for OneBotActionClient<'a> {
    fn clone(&self) -> Self {
        Self { ctx: self.ctx }
    }
}

impl<'a> OneBotActionClient<'a> {
    /// Create a new client backed by the given runtime context.
    pub fn new(ctx: &'a dyn RuntimeBotContext) -> Self {
        Self { ctx }
    }

    /// Get the bot's own login info (user_id and nickname).
    pub async fn get_login_info(&self) -> Result<LoginInfoResponse> {
        let response = self.send_onebot_action("get_login_info", Value::Object(Default::default())).await?;
        parse_action_data(response)
    }

    /// Retrieve a message by its ID.
    pub async fn get_msg(&self, message_id: i64) -> Result<GetMsgResponse> {
        let response = self
            .send_onebot_action(
                "get_msg",
                serde_json::json!({
                    "message_id": message_id,
                }),
            )
            .await?;
        parse_action_data(response)
    }

    /// Delete (recall) a message by its ID.
    pub async fn delete_msg(&self, message_id: i64) -> Result<()> {
        let response = self
            .send_onebot_action(
                "delete_msg",
                serde_json::json!({
                    "message_id": message_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    /// Get information about a group. Set `no_cache` to bypass the cache.
    pub async fn get_group_info(&self, group_id: i64, no_cache: bool) -> Result<GroupInfoResponse> {
        let response = self
            .send_onebot_action(
                "get_group_info",
                serde_json::json!({
                    "group_id": group_id,
                    "no_cache": no_cache,
                }),
            )
            .await?;
        parse_action_data(response)
    }

    /// Get the list of groups the bot has joined.
    pub async fn get_group_list(&self) -> Result<Vec<GroupInfoResponse>> {
        let response = self.send_onebot_action("get_group_list", Value::Object(Default::default())).await?;
        parse_action_data(response)
    }

    /// Get info about a specific group member.
    pub async fn get_group_member_info(
        &self,
        group_id: i64,
        user_id: i64,
        no_cache: bool,
    ) -> Result<GroupMemberInfoResponse> {
        let response = self
            .send_onebot_action(
                "get_group_member_info",
                serde_json::json!({
                    "group_id": group_id,
                    "user_id": user_id,
                    "no_cache": no_cache,
                }),
            )
            .await?;
        parse_action_data(response)
    }

    /// Get the member list for a group.
    pub async fn get_group_member_list(&self, group_id: i64) -> Result<Vec<GroupMemberInfoResponse>> {
        let response = self
            .send_onebot_action(
                "get_group_member_list",
                serde_json::json!({
                    "group_id": group_id,
                }),
            )
            .await?;
        parse_action_data(response)
    }

    // ── Message actions ──

    /// Send a private (direct) message to a user.
    pub async fn send_private_msg(&self, user_id: i64, message: Message) -> Result<SendMsgResponse> {
        let response = self
            .send_onebot_action(
                "send_private_msg",
                serde_json::json!({
                    "user_id": user_id,
                    "message": message.to_onebot_value(),
                }),
            )
            .await?;
        parse_action_data(response)
    }

    /// Send a message to a group.
    pub async fn send_group_msg(&self, group_id: i64, message: Message) -> Result<SendMsgResponse> {
        let response = self
            .send_onebot_action(
                "send_group_msg",
                serde_json::json!({
                    "group_id": group_id,
                    "message": message.to_onebot_value(),
                }),
            )
            .await?;
        parse_action_data(response)
    }

    /// Send a message by type (`"private"` or `"group"`) and target ID.
    pub async fn send_msg(
        &self,
        message_type: &str,
        id: i64,
        message: Message,
    ) -> Result<SendMsgResponse> {
        let mut params = serde_json::json!({
            "message_type": message_type,
            "message": message.to_onebot_value(),
        });
        if message_type == "private" {
            params["user_id"] = serde_json::json!(id);
        } else {
            params["group_id"] = serde_json::json!(id);
        }
        let response = self.send_onebot_action("send_msg", params).await?;
        parse_action_data(response)
    }

    pub async fn get_forward_msg(&self, id: impl Into<String>) -> Result<GetForwardMsgResponse> {
        let response = self
            .send_onebot_action(
                "get_forward_msg",
                serde_json::json!({ "id": id.into() }),
            )
            .await?;
        parse_action_data(response)
    }

    pub async fn send_like(&self, user_id: i64, times: i32) -> Result<()> {
        let response = self
            .send_onebot_action(
                "send_like",
                serde_json::json!({
                    "user_id": user_id,
                    "times": times,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    // ── Group admin actions ──

    pub async fn set_group_kick(
        &self,
        group_id: i64,
        user_id: i64,
        reject_add_request: bool,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_kick",
                serde_json::json!({
                    "group_id": group_id,
                    "user_id": user_id,
                    "reject_add_request": reject_add_request,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_ban(
        &self,
        group_id: i64,
        user_id: i64,
        duration: i64,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_ban",
                serde_json::json!({
                    "group_id": group_id,
                    "user_id": user_id,
                    "duration": duration,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_anonymous_ban(
        &self,
        group_id: i64,
        anonymous_flag: &str,
        duration: i64,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_anonymous_ban",
                serde_json::json!({
                    "group_id": group_id,
                    "anonymous_flag": anonymous_flag,
                    "duration": duration,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_whole_ban(&self, group_id: i64, enable: bool) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_whole_ban",
                serde_json::json!({
                    "group_id": group_id,
                    "enable": enable,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_admin(
        &self,
        group_id: i64,
        user_id: i64,
        enable: bool,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_admin",
                serde_json::json!({
                    "group_id": group_id,
                    "user_id": user_id,
                    "enable": enable,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_card(
        &self,
        group_id: i64,
        user_id: i64,
        card: &str,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_card",
                serde_json::json!({
                    "group_id": group_id,
                    "user_id": user_id,
                    "card": card,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_name(&self, group_id: i64, group_name: &str) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_name",
                serde_json::json!({
                    "group_id": group_id,
                    "group_name": group_name,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_leave(&self, group_id: i64, is_dismiss: bool) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_leave",
                serde_json::json!({
                    "group_id": group_id,
                    "is_dismiss": is_dismiss,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_special_title(
        &self,
        group_id: i64,
        user_id: i64,
        special_title: &str,
        duration: i64,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_special_title",
                serde_json::json!({
                    "group_id": group_id,
                    "user_id": user_id,
                    "special_title": special_title,
                    "duration": duration,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_anonymous(&self, group_id: i64, enable: bool) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_anonymous",
                serde_json::json!({
                    "group_id": group_id,
                    "enable": enable,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_portrait(&self, group_id: i64, file: &str, cache: i32) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_portrait",
                serde_json::json!({
                    "group_id": group_id,
                    "file": file,
                    "cache": cache,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    // ── Request handling actions ──

    pub async fn set_friend_add_request(
        &self,
        flag: &str,
        approve: bool,
        remark: Option<&str>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "flag": flag,
            "approve": approve,
        });
        if let Some(remark) = remark {
            params["remark"] = serde_json::json!(remark);
        }
        let response = self
            .send_onebot_action("set_friend_add_request", params)
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_group_add_request(
        &self,
        flag: &str,
        sub_type: &str,
        approve: bool,
        reason: Option<&str>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "flag": flag,
            "sub_type": sub_type,
            "approve": approve,
        });
        if let Some(reason) = reason {
            params["reason"] = serde_json::json!(reason);
        }
        let response = self
            .send_onebot_action("set_group_add_request", params)
            .await?;
        ensure_action_ok(&response)
    }

    // ── Info query actions ──

    pub async fn get_stranger_info(
        &self,
        user_id: i64,
        no_cache: bool,
    ) -> Result<StrangerInfoResponse> {
        let response = self
            .send_onebot_action(
                "get_stranger_info",
                serde_json::json!({
                    "user_id": user_id,
                    "no_cache": no_cache,
                }),
            )
            .await?;
        parse_action_data(response)
    }

    pub async fn get_friend_list(&self) -> Result<Vec<FriendInfoResponse>> {
        let response = self
            .send_onebot_action("get_friend_list", Value::Object(Default::default()))
            .await?;
        parse_action_data(response)
    }

    pub async fn get_group_honor_info(
        &self,
        group_id: i64,
        honor_type: &str,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_group_honor_info",
                serde_json::json!({
                    "group_id": group_id,
                    "type": honor_type,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn can_send_image(&self) -> Result<CanSendResponse> {
        let response = self
            .send_onebot_action("can_send_image", Value::Object(Default::default()))
            .await?;
        parse_action_data(response)
    }

    pub async fn can_send_record(&self) -> Result<CanSendResponse> {
        let response = self
            .send_onebot_action("can_send_record", Value::Object(Default::default()))
            .await?;
        parse_action_data(response)
    }

    pub async fn get_status(&self) -> Result<Value> {
        let response = self
            .send_onebot_action("get_status", Value::Object(Default::default()))
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_version_info(&self) -> Result<Value> {
        let response = self
            .send_onebot_action("get_version_info", Value::Object(Default::default()))
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    // ── Go-CQHTTP extended actions ──

    pub async fn delete_friend(&self, user_id: i64) -> Result<()> {
        let response = self
            .send_onebot_action(
                "delete_friend",
                serde_json::json!({
                    "user_id": user_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn delete_unidirectional_friend(&self, user_id: i64) -> Result<()> {
        let response = self
            .send_onebot_action(
                "delete_unidirectional_friend",
                serde_json::json!({
                    "user_id": user_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn send_group_notice(&self, group_id: i64, content: &str) -> Result<()> {
        let response = self
            .send_onebot_action(
                "_send_group_notice",
                serde_json::json!({
                    "group_id": group_id,
                    "content": content,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn get_group_at_all_remain(&self, group_id: i64) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_group_at_all_remain",
                serde_json::json!({
                    "group_id": group_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn send_group_sign(&self, group_id: i64) -> Result<()> {
        let response = self
            .send_onebot_action(
                "send_group_sign",
                serde_json::json!({
                    "group_id": group_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_essence_msg(&self, message_id: i64) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_essence_msg",
                serde_json::json!({
                    "message_id": message_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn delete_essence_msg(&self, message_id: i64) -> Result<()> {
        let response = self
            .send_onebot_action(
                "delete_essence_msg",
                serde_json::json!({
                    "message_id": message_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn get_essence_msg_list(&self, group_id: i64) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_essence_msg_list",
                serde_json::json!({
                    "group_id": group_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn check_url_safely(&self, url: &str) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "check_url_safely",
                serde_json::json!({
                    "url": url,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_online_clients(&self, no_cache: bool) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_online_clients",
                serde_json::json!({
                    "no_cache": no_cache,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    // ── File operations ──

    pub async fn upload_group_file(
        &self,
        group_id: i64,
        file: &str,
        name: &str,
        folder: Option<&str>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "group_id": group_id,
            "file": file,
            "name": name,
        });
        if let Some(folder) = folder {
            params["folder"] = serde_json::json!(folder);
        }
        let response = self
            .send_onebot_action("upload_group_file", params)
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn upload_private_file(
        &self,
        user_id: i64,
        file: &str,
        name: &str,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "upload_private_file",
                serde_json::json!({
                    "user_id": user_id,
                    "file": file,
                    "name": name,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn get_group_root_files(&self, group_id: i64) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_group_root_files",
                serde_json::json!({
                    "group_id": group_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_group_files_by_folder(
        &self,
        group_id: i64,
        folder_id: &str,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_group_files_by_folder",
                serde_json::json!({
                    "group_id": group_id,
                    "folder_id": folder_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_group_file_url(
        &self,
        group_id: i64,
        file_id: &str,
        busid: i32,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_group_file_url",
                serde_json::json!({
                    "group_id": group_id,
                    "file_id": file_id,
                    "busid": busid,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn create_group_file_folder(
        &self,
        group_id: i64,
        name: &str,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "create_group_file_folder",
                serde_json::json!({
                    "group_id": group_id,
                    "name": name,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn delete_group_folder(
        &self,
        group_id: i64,
        folder_id: &str,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "delete_group_folder",
                serde_json::json!({
                    "group_id": group_id,
                    "folder_id": folder_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn delete_group_file(
        &self,
        group_id: i64,
        file_id: &str,
        busid: i32,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "delete_group_file",
                serde_json::json!({
                    "group_id": group_id,
                    "file_id": file_id,
                    "busid": busid,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    // ── Forward messages ──

    pub async fn send_group_forward_msg(
        &self,
        group_id: i64,
        messages: Value,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "send_group_forward_msg",
                serde_json::json!({
                    "group_id": group_id,
                    "messages": messages,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn send_private_forward_msg(
        &self,
        user_id: i64,
        messages: Value,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "send_private_forward_msg",
                serde_json::json!({
                    "user_id": user_id,
                    "messages": messages,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    // ── Guild operations ──

    pub async fn send_guild_channel_msg(
        &self,
        guild_id: &str,
        channel_id: &str,
        message: Message,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "send_guild_channel_msg",
                serde_json::json!({
                    "guild_id": guild_id,
                    "channel_id": channel_id,
                    "message": message.to_onebot_value(),
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_guild_list(&self) -> Result<Value> {
        let response = self
            .send_onebot_action("get_guild_list", Value::Object(Default::default()))
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_guild_channel_list(
        &self,
        guild_id: &str,
        no_cache: bool,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_guild_channel_list",
                serde_json::json!({
                    "guild_id": guild_id,
                    "no_cache": no_cache,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_guild_member_list(
        &self,
        guild_id: &str,
        next_token: &str,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_guild_member_list",
                serde_json::json!({
                    "guild_id": guild_id,
                    "next_token": next_token,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_guild_member_profile(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "get_guild_member_profile",
                serde_json::json!({
                    "guild_id": guild_id,
                    "user_id": user_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    // ── Misc ──

    pub async fn download_file(
        &self,
        url: &str,
        thread_count: i32,
        headers: &str,
    ) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "download_file",
                serde_json::json!({
                    "url": url,
                    "thread_count": thread_count,
                    "headers": headers,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn ocr_image(&self, image: &str) -> Result<Value> {
        let response = self
            .send_onebot_action(
                "ocr_image",
                serde_json::json!({
                    "image": image,
                }),
            )
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn set_qq_profile(
        &self,
        nickname: &str,
        company: &str,
        email: &str,
        college: &str,
        personal_note: &str,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_qq_profile",
                serde_json::json!({
                    "nickname": nickname,
                    "company": company,
                    "email": email,
                    "college": college,
                    "personal_note": personal_note,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    // ── NapCat / Lagrange extended actions ──

    pub async fn get_group_msg_history(
        &self,
        group_id: i64,
        message_seq: Option<i64>,
        count: i32,
    ) -> Result<Value> {
        let mut params = serde_json::json!({ "group_id": group_id, "count": count });
        if let Some(seq) = message_seq {
            params["message_seq"] = serde_json::json!(seq);
        }
        let response = self
            .send_onebot_action("get_group_msg_history", params)
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn get_friend_msg_history(
        &self,
        user_id: i64,
        message_seq: Option<i64>,
        count: i32,
    ) -> Result<Value> {
        let mut params = serde_json::json!({ "user_id": user_id, "count": count });
        if let Some(seq) = message_seq {
            params["message_seq"] = serde_json::json!(seq);
        }
        let response = self
            .send_onebot_action("get_friend_msg_history", params)
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn send_group_poke(&self, group_id: i64, user_id: i64) -> Result<()> {
        let response = self
            .send_onebot_action(
                "group_poke",
                serde_json::json!({
                    "group_id": group_id,
                    "user_id": user_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn send_friend_poke(&self, user_id: i64) -> Result<()> {
        let response = self
            .send_onebot_action(
                "friend_poke",
                serde_json::json!({
                    "user_id": user_id,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn set_msg_emoji_like(
        &self,
        message_id: i64,
        emoji_id: &str,
        set: bool,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_msg_emoji_like",
                serde_json::json!({
                    "message_id": message_id,
                    "emoji_id": emoji_id,
                    "set": set,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    pub async fn fetch_custom_face(&self) -> Result<Value> {
        let response = self
            .send_onebot_action("fetch_custom_face", Value::Object(Default::default()))
            .await?;
        ensure_action_ok(&response)?;
        Ok(response.data)
    }

    pub async fn set_group_reaction(
        &self,
        group_id: i64,
        message_id: i64,
        code: &str,
        is_add: bool,
    ) -> Result<()> {
        let response = self
            .send_onebot_action(
                "set_group_reaction",
                serde_json::json!({
                    "group_id": group_id,
                    "message_id": message_id,
                    "code": code,
                    "is_add": is_add,
                }),
            )
            .await?;
        ensure_action_ok(&response)
    }

    /// Send a custom/arbitrary OneBot action. This is a passthrough for actions
    /// not covered by the built-in methods (e.g., implementation-specific extensions).
    pub async fn custom_action(&self, action: &str, params: serde_json::Value) -> Result<NormalizedActionResponse> {
        self.send_onebot_action(action, params).await
    }

    async fn send_onebot_action(
        &self,
        action: &str,
        params: Value,
    ) -> Result<NormalizedActionResponse> {
        ensure_onebot11(self.ctx)?;
        self.ctx
            .send_action(NormalizedActionRequest {
                protocol: ProtocolId::OneBot11,
                bot_instance: self.ctx.bot_instance().to_string(),
                action: action.to_string(),
                params,
                echo: None,
                timeout_ms: 5000,
                metadata: ActionMeta {
                    source: format!("onebot-action-client:{action}"),
                },
            })
            .await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoginInfoResponse {
    pub user_id: i64,
    pub nickname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SenderInfo {
    pub user_id: Option<i64>,
    pub nickname: Option<String>,
    pub card: Option<String>,
    pub sex: Option<String>,
    pub age: Option<i64>,
    pub area: Option<String>,
    pub level: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetMsgResponse {
    pub message_id: i64,
    pub real_id: Option<i64>,
    pub sender: Option<SenderInfo>,
    pub time: Option<i64>,
    pub message: Value,
    pub raw_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupInfoResponse {
    pub group_id: i64,
    pub group_name: String,
    pub member_count: Option<i64>,
    pub max_member_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupMemberInfoResponse {
    pub group_id: Option<i64>,
    pub user_id: i64,
    pub nickname: Option<String>,
    pub card: Option<String>,
    pub sex: Option<String>,
    pub age: Option<i64>,
    pub area: Option<String>,
    pub join_time: Option<i64>,
    pub last_sent_time: Option<i64>,
    pub level: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SendMsgResponse {
    pub message_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrangerInfoResponse {
    pub user_id: i64,
    pub nickname: String,
    pub sex: Option<String>,
    pub age: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FriendInfoResponse {
    pub user_id: i64,
    pub nickname: String,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetForwardMsgResponse {
    pub message: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanSendResponse {
    pub yes: bool,
}

fn ensure_onebot11(ctx: &dyn RuntimeBotContext) -> Result<()> {
    if ctx.protocol() != ProtocolId::OneBot11 {
        return Err(qimen_error::QimenError::Protocol(format!(
            "OneBotActionClient requires OneBot11 protocol, got {:?}",
            ctx.protocol()
        )));
    }
    Ok(())
}

fn ensure_action_ok(response: &NormalizedActionResponse) -> Result<()> {
    match response.status {
        qimen_protocol_core::ActionStatus::Ok | qimen_protocol_core::ActionStatus::Async => Ok(()),
        qimen_protocol_core::ActionStatus::Failed => Err(qimen_error::QimenError::Runtime(
            format!("OneBot action failed with retcode {}", response.retcode),
        )),
    }
}

fn parse_action_data<T: DeserializeOwned>(response: NormalizedActionResponse) -> Result<T> {
    ensure_action_ok(&response)?;
    serde_json::from_value(response.data).map_err(Into::into)
}

/// Metadata describing a plugin: identity, version, and compatibility constraints.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub api_version: &'static str,
    pub compatibility: PluginCompatibility,
}

/// Version constraints that describe which host/framework versions a plugin supports.
#[derive(Debug, Clone)]
pub struct PluginCompatibility {
    pub host_api: &'static str,
    pub framework_min: &'static str,
    pub framework_max: &'static str,
}

/// Declares a single command that a [`CommandPlugin`] handles.
///
/// Includes the command name, aliases, usage examples, required permission
/// level, and an optional [`MessageFilter`] for advanced matching.
#[derive(Debug, Clone)]
pub struct CommandDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub aliases: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub category: &'static str,
    pub hidden: bool,
    pub required_role: CommandRole,
    pub filter: Option<MessageFilter>,
}

impl CommandDefinition {
    /// Create a new `CommandDefinition` with sensible defaults.
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            aliases: &[],
            examples: &[],
            category: "general",
            hidden: false,
            required_role: CommandRole::Anyone,
            filter: None,
        }
    }

    /// Set command aliases.
    pub const fn aliases(mut self, aliases: &'static [&'static str]) -> Self {
        self.aliases = aliases;
        self
    }

    /// Set usage examples.
    pub const fn examples(mut self, examples: &'static [&'static str]) -> Self {
        self.examples = examples;
        self
    }

    /// Set the command category.
    pub const fn category(mut self, category: &'static str) -> Self {
        self.category = category;
        self
    }

    /// Mark the command as hidden (not shown in help).
    pub const fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Set the required permission role.
    pub const fn role(mut self, role: CommandRole) -> Self {
        self.required_role = role;
        self
    }

    /// Set a message filter for advanced matching.
    pub fn filter(mut self, filter: MessageFilter) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// At-bot detection mode (matches Shiro's AtEnum)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AtMode {
    /// Require the message to @bot
    Need,
    /// Require the message to NOT @bot
    NotNeed,
    /// Don't care about @bot status
    #[default]
    Both,
}

/// Reply filtering mode
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ReplyFilter {
    /// No reply filtering
    #[default]
    None,
    /// Must be a reply to the bot's message
    ReplyMe,
    /// Must be a reply to someone else's message
    ReplyOther,
}

/// Media type filter
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Record,
    Video,
}

/// Declarative filter for matching incoming messages.
///
/// Supports regex matching (`cmd`), prefix/suffix/substring checks, group and
/// sender whitelists, @-bot detection, reply filtering, and media type requirements.
/// Set `invert` to negate the overall result.
#[derive(Debug, Clone, Default)]
pub struct MessageFilter {
    pub cmd: Option<String>,
    pub starts_with: Option<String>,
    pub ends_with: Option<String>,
    pub contains: Option<String>,
    pub groups: Vec<i64>,
    pub senders: Vec<i64>,
    pub at_mode: AtMode,
    pub reply_filter: ReplyFilter,
    pub media_types: Vec<MediaType>,
    pub invert: bool,
}

/// Result of evaluating a [`MessageFilter`] against a message event.
pub struct MatchResult {
    pub matched: bool,
    pub captures: Vec<String>,
}

/// Configuration for the per-plugin token-bucket rate limiter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_rate")]
    pub rate: f64,
    #[serde(default = "default_capacity")]
    pub capacity: u32,
    #[serde(default)]
    pub timeout_secs: u64,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            enable: false,
            rate: default_rate(),
            capacity: default_capacity(),
            timeout_secs: 0,
        }
    }
}

fn default_rate() -> f64 {
    5.0
}

fn default_capacity() -> u32 {
    10
}

/// Interceptor that can inspect or block message events before/after dispatch.
///
/// Return `false` from [`pre_handle`](Self::pre_handle) to stop the event
/// from reaching any plugin. `after_completion` runs in reverse order after
/// all plugins have finished processing.
#[async_trait]
pub trait MessageEventInterceptor: Send + Sync {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        let _ = (bot_id, event);
        true
    }
    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        let _ = (bot_id, event);
    }
}

/// Permission level required to invoke a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRole {
    /// Any user may invoke the command.
    Anyone,
    /// Only group admins (or higher) may invoke the command.
    Admin,
    /// Only the bot owner may invoke the command.
    Owner,
}

/// A parsed command invocation passed to [`CommandPlugin::on_command`].
///
/// Contains the matched [`CommandDefinition`], extracted arguments, and the
/// original message text.
#[derive(Debug, Clone)]
pub struct CommandInvocation {
    pub definition: CommandDefinition,
    pub args: Vec<String>,
    pub source_text: String,
}

#[derive(Debug, Clone)]
pub enum BuiltinCommandAction {
    Help,
    PluginsShow,
    PluginsEnable { plugin_id: String },
    PluginsDisable { plugin_id: String },
    PluginsReload,
    RegistryReport,
    RegistryConflicts,
    DynamicErrors,
    DynamicErrorsClear,
}

/// Signal returned by a [`CommandPlugin`] to control dispatch flow.
#[derive(Debug, Clone)]
pub enum CommandPluginSignal {
    /// Send a reply and continue to the next plugin in the chain.
    Reply(Message),
    /// Do nothing and continue to the next plugin.
    Continue,
    /// Stop the plugin chain - no further plugins will process this command.
    /// The reply (if any) from THIS plugin is the final response.
    Block(Message),
    /// Silently stop the plugin chain without sending any reply.
    Ignore,
}

impl From<Message> for CommandPluginSignal {
    fn from(msg: Message) -> Self {
        Self::Reply(msg)
    }
}

impl From<String> for CommandPluginSignal {
    fn from(s: String) -> Self {
        Self::Reply(Message::text(s))
    }
}

impl From<&str> for CommandPluginSignal {
    fn from(s: &str) -> Self {
        Self::Reply(Message::text(s))
    }
}

// ─── IntoCommandSignal / IntoSystemSignal ──────────────────────────────

/// Converts a return value into [`CommandPluginSignal`].
///
/// Implemented for `CommandPluginSignal`, `Message`, `String`, `&str`,
/// and `Result<T, E>` where `T: IntoCommandSignal` and `E: Display`.
pub trait IntoCommandSignal {
    fn into_signal(self) -> CommandPluginSignal;
}

impl IntoCommandSignal for CommandPluginSignal {
    fn into_signal(self) -> CommandPluginSignal {
        self
    }
}

impl IntoCommandSignal for Message {
    fn into_signal(self) -> CommandPluginSignal {
        CommandPluginSignal::Reply(self)
    }
}

impl IntoCommandSignal for String {
    fn into_signal(self) -> CommandPluginSignal {
        CommandPluginSignal::Reply(Message::text(self))
    }
}

impl IntoCommandSignal for &str {
    fn into_signal(self) -> CommandPluginSignal {
        CommandPluginSignal::Reply(Message::text(self))
    }
}

impl<T: IntoCommandSignal, E: std::fmt::Display> IntoCommandSignal
    for std::result::Result<T, E>
{
    fn into_signal(self) -> CommandPluginSignal {
        match self {
            Ok(v) => v.into_signal(),
            Err(e) => CommandPluginSignal::Reply(Message::text(format!("Error: {e}"))),
        }
    }
}

/// Converts a return value into [`SystemPluginSignal`].
///
/// Implemented for `SystemPluginSignal`, `Message`, `String`, `&str`,
/// and `Result<T, E>` where `T: IntoSystemSignal` and `E: Display`.
pub trait IntoSystemSignal {
    fn into_signal(self) -> SystemPluginSignal;
}

impl IntoSystemSignal for SystemPluginSignal {
    fn into_signal(self) -> SystemPluginSignal {
        self
    }
}

impl IntoSystemSignal for Message {
    fn into_signal(self) -> SystemPluginSignal {
        SystemPluginSignal::Reply(self)
    }
}

impl IntoSystemSignal for String {
    fn into_signal(self) -> SystemPluginSignal {
        SystemPluginSignal::Reply(Message::text(self))
    }
}

impl IntoSystemSignal for &str {
    fn into_signal(self) -> SystemPluginSignal {
        SystemPluginSignal::Reply(Message::text(self))
    }
}

impl<T: IntoSystemSignal, E: std::fmt::Display> IntoSystemSignal
    for std::result::Result<T, E>
{
    fn into_signal(self) -> SystemPluginSignal {
        match self {
            Ok(v) => v.into_signal(),
            Err(e) => SystemPluginSignal::Reply(Message::text(format!("Error: {e}"))),
        }
    }
}

/// Routing key for notice-type system events, identifying the specific notice sub-type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemNoticeRoute {
    GroupUpload,
    GroupAdminSet,
    GroupAdminUnset,
    GroupDecreaseLeave,
    GroupDecreaseKick,
    GroupDecreaseKickMe,
    GroupIncreaseApprove,
    GroupIncreaseInvite,
    GroupBanBan,
    GroupBanLiftBan,
    FriendAdd,
    GroupRecall,
    FriendRecall,
    GroupPoke,
    PrivatePoke,
    NotifyLuckyKing,
    NotifyHonor,
    GroupCard,
    OfflineFile,
    ClientStatus,
    EssenceAdd,
    EssenceDelete,
    GroupReaction,
    MessageEmojiLike,
    ChannelCreated,
    ChannelDestroyed,
    ChannelUpdated,
    GuildMessageReactionsUpdated,
    Unknown(String),
}

/// Routing key for request-type system events (friend requests, group join/invite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemRequestRoute {
    Friend,
    GroupAdd,
    GroupInvite,
    Unknown {
        request_type: String,
        sub_type: Option<String>,
    },
}

/// Routing key for meta-type system events (lifecycle, heartbeat).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMetaRoute {
    LifecycleEnable,
    LifecycleDisable,
    LifecycleConnect,
    LifecycleOther(String),
    Heartbeat,
    Unknown(String),
}

/// Signal returned by a [`SystemPlugin`] to control dispatch flow.
#[derive(Debug, Clone)]
pub enum SystemPluginSignal {
    /// Do nothing and continue to the next plugin.
    Continue,
    /// Send a reply message and continue.
    Reply(Message),
    ApproveFriend {
        flag: String,
        remark: Option<String>,
    },
    RejectFriend {
        flag: String,
        reason: Option<String>,
    },
    ApproveGroupInvite {
        flag: String,
        sub_type: String,
    },
    RejectGroupInvite {
        flag: String,
        sub_type: String,
        reason: Option<String>,
    },
    /// Stop the plugin chain - no further plugins will process this event.
    /// The reply (if any) from THIS plugin is the final response.
    Block(Message),
    /// Silently stop the plugin chain without sending any reply.
    Ignore,
}

/// Context passed to [`CommandPlugin::on_command`] with the current bot, event, and runtime.
pub struct CommandPluginContext<'a> {
    pub bot_id: &'a str,
    pub event: &'a NormalizedEvent,
    pub runtime: &'a dyn RuntimeBotContext,
}

impl<'a> CommandPluginContext<'a> {
    pub fn onebot_actions(&self) -> OneBotActionClient<'a> {
        OneBotActionClient::new(self.runtime)
    }

    // ── Convenience forwarding from event ──

    /// Sender user ID as `&str` (from `actor.id`).
    pub fn sender_id(&self) -> Option<&str> {
        self.event.sender_id()
    }

    /// Sender user ID as `i64`.
    pub fn sender_id_i64(&self) -> Option<i64> {
        self.event.sender_id_i64()
    }

    /// Chat ID from `chat.id`.
    pub fn chat_id(&self) -> Option<&str> {
        self.event.chat_id()
    }

    /// Group ID as `&str` (only when chat is a group).
    pub fn group_id(&self) -> Option<&str> {
        self.event.group_id()
    }

    /// Group ID as `i64` from raw_json.
    pub fn group_id_i64(&self) -> Option<i64> {
        self.event.group_id_i64()
    }

    /// Whether the event is from a group chat.
    pub fn is_group(&self) -> bool {
        self.event.is_group()
    }

    /// Whether the event is from a private chat.
    pub fn is_private(&self) -> bool {
        self.event.is_private()
    }

    /// Shortcut to `event.plain_text()`.
    pub fn plain_text(&self) -> String {
        self.event.plain_text()
    }

    /// Access the parsed message, if present.
    pub fn message(&self) -> Option<&Message> {
        self.event.message.as_ref()
    }
}

/// Context passed to [`SystemPlugin`] handlers with the current bot, raw event JSON, and runtime.
pub struct SystemPluginContext<'a> {
    pub bot_id: &'a str,
    pub event: &'a Value,
    pub runtime: &'a dyn RuntimeBotContext,
}

impl<'a> SystemPluginContext<'a> {
    pub fn onebot_actions(&self) -> OneBotActionClient<'a> {
        OneBotActionClient::new(self.runtime)
    }
}

/// A plugin that handles text commands from users.
///
/// Implement this trait to create command-based plugins. Each plugin
/// declares its commands via [`commands()`](Self::commands) and handles
/// invocations in [`on_command()`](Self::on_command).
#[async_trait]
pub trait CommandPlugin: Send + Sync {
    /// Return metadata identifying this plugin.
    fn metadata(&self) -> PluginMetadata;
    /// Declare the commands this plugin handles.
    fn commands(&self) -> Vec<CommandDefinition>;

    /// Plugin priority. Lower values execute first. Default is 100.
    /// Builtin handlers use 10, this allows plugins to run before or after builtins.
    fn priority(&self) -> i32 {
        100
    }

    fn is_dynamic(&self) -> bool {
        false
    }

    async fn on_command(
        &self,
        _ctx: &CommandPluginContext<'_>,
        _invocation: &CommandInvocation,
    ) -> Option<CommandPluginSignal> {
        None
    }
}

/// A plugin that handles system events: notices, requests, and meta events.
///
/// Implement one or more of [`on_notice`](Self::on_notice),
/// [`on_request`](Self::on_request), or [`on_meta`](Self::on_meta)
/// to react to non-message events such as group member changes,
/// friend requests, or heartbeat signals.
#[async_trait]
pub trait SystemPlugin: Send + Sync {
    /// Return metadata identifying this plugin.
    fn metadata(&self) -> PluginMetadata;

    /// Plugin priority. Lower values execute first. Default is 100.
    /// Builtin handlers use 10, this allows plugins to run before or after builtins.
    fn priority(&self) -> i32 {
        100
    }

    fn is_dynamic(&self) -> bool {
        false
    }

    async fn on_notice(
        &self,
        _ctx: &SystemPluginContext<'_>,
        _route: &SystemNoticeRoute,
    ) -> Option<SystemPluginSignal> {
        None
    }

    async fn on_request(
        &self,
        _ctx: &SystemPluginContext<'_>,
        _route: &SystemRequestRoute,
    ) -> Option<SystemPluginSignal> {
        None
    }

    async fn on_meta(
        &self,
        _ctx: &SystemPluginContext<'_>,
        _route: &SystemMetaRoute,
    ) -> Option<SystemPluginSignal> {
        None
    }
}

#[derive(Clone)]
pub struct PluginRegistration {
    pub module_id: &'static str,
    pub command_plugins: Vec<Arc<dyn CommandPlugin>>,
    pub system_plugins: Vec<Arc<dyn SystemPlugin>>,
    pub interceptors: Vec<Arc<dyn MessageEventInterceptor>>,
}

pub trait PluginRegistrar {
    fn register(&mut self, registration: PluginRegistration);
}

/// A collection of command and system plugins, typically produced by a [`Module`].
#[derive(Default)]
pub struct PluginBundle {
    pub command_plugins: Vec<Arc<dyn CommandPlugin>>,
    pub system_plugins: Vec<Arc<dyn SystemPlugin>>,
    pub interceptors: Vec<Arc<dyn MessageEventInterceptor>>,
}

/// A loadable module that groups related plugins together.
///
/// Modules have a lifecycle (`on_load` / `on_unload`) and provide their
/// plugins via [`command_plugins`](Self::command_plugins) and
/// [`system_plugins`](Self::system_plugins).
#[async_trait]
pub trait Module: Send + Sync {
    /// Unique module identifier.
    fn id(&self) -> &'static str;
    /// Called when the module is loaded. Perform initialization here.
    async fn on_load(&self) -> Result<()>;

    /// Called when the module is being disabled/unloaded.
    async fn on_unload(&self) -> Result<()> {
        Ok(())
    }

    /// Check if the module supports hot reload.
    fn supports_hot_reload(&self) -> bool {
        false
    }

    fn command_plugins(&self) -> Vec<Arc<dyn CommandPlugin>> {
        Vec::new()
    }

    fn system_plugins(&self) -> Vec<Arc<dyn SystemPlugin>> {
        Vec::new()
    }

    fn interceptors(&self) -> Vec<Arc<dyn MessageEventInterceptor>> {
        Vec::new()
    }

    fn register_plugins(&self, registrar: &mut dyn PluginRegistrar) {
        registrar.register(PluginRegistration {
            module_id: self.id(),
            command_plugins: self.command_plugins(),
            system_plugins: self.system_plugins(),
            interceptors: self.interceptors(),
        });
    }
}

/// Entry registered via `inventory` by the `#[module]` macro.
///
/// Each plugin crate submits one of these at link time so the official host
/// can discover all compiled-in modules without manual match branches.
pub struct ModuleEntry {
    pub id: &'static str,
    pub factory: fn() -> Box<dyn Module>,
}
inventory::collect!(ModuleEntry);

/// Convenience re-exports for plugin development.
///
/// Import everything you need for a typical plugin with a single `use`:
/// ```rust,ignore
/// use qimen_plugin_api::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{
        CommandDefinition, CommandInvocation, CommandPlugin, CommandPluginContext,
        CommandPluginSignal, CommandRole, IntoCommandSignal, IntoSystemSignal,
        MessageEventInterceptor, Module, ModuleEntry, OneBotActionClient, PluginCompatibility, PluginMetadata,
        RuntimeBotContext, SystemMetaRoute, SystemNoticeRoute, SystemPlugin, SystemPluginContext,
        SystemPluginSignal, SystemRequestRoute,
    };
    pub use async_trait::async_trait;
    pub use qimen_error::Result;
    pub use qimen_message::Message;
    pub use qimen_protocol_core::{
        NormalizedEvent, qq_avatar_url, qq_group_avatar_url, value_to_lossless_id,
    };
    pub use qimen_plugin_derive::{commands, module, qimen_commands, qimen_module, system};
    pub use std::sync::Arc;
}
