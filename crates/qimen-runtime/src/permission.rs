use qimen_protocol_core::NormalizedEvent;

use crate::BotRuntimeInfo;

#[derive(Debug, Clone, Copy)]
pub struct PermissionState {
    pub is_admin: bool,
    pub is_owner: bool,
}

pub struct PermissionResolver;

impl PermissionResolver {
    pub fn resolve(bot: &BotRuntimeInfo, event: &NormalizedEvent) -> PermissionState {
        let user_id = event.sender_id().unwrap_or("");
        let is_owner = bot.owners.iter().any(|owner| owner == user_id);
        let is_admin = is_owner
            || bot.admins.iter().any(|admin| admin == user_id)
            || event.is_group_admin_or_owner();

        PermissionState { is_admin, is_owner }
    }
}
