pub(crate) fn plugin_allowed(enabled_plugins: Option<&[String]>, plugin_id: &str) -> bool {
    enabled_plugins.is_none_or(|enabled| enabled.iter().any(|id| id == plugin_id))
}
