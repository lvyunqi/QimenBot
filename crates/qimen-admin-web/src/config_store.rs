use crate::error::AdminError;
use crate::types::{BotMutation, GeneralMutation, RevisionView};
use chrono::{SecondsFormat, Utc};
use qimen_config::AppConfig;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, Value, value};

#[derive(Debug)]
pub struct StoredConfig {
    pub config: AppConfig,
    pub revision: String,
}

#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
    revisions_dir: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let revisions_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".qimen-revisions");
        Self {
            path,
            revisions_dir,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn read(&self) -> Result<StoredConfig, AdminError> {
        let _guard = self.lock.lock().await;
        self.read_unlocked()
    }

    pub async fn update_general(
        &self,
        expected_revision: &str,
        update: &GeneralMutation,
    ) -> Result<StoredConfig, AdminError> {
        let _guard = self.lock.lock().await;
        let raw = fs::read_to_string(&self.path)?;
        self.ensure_revision(&raw, expected_revision)?;
        let mut document = raw.parse::<DocumentMut>()?;
        apply_general(&mut document, update);
        self.save_document(raw, document)
    }

    pub async fn save_bot(
        &self,
        existing_id: Option<&str>,
        expected_revision: &str,
        update: &BotMutation,
    ) -> Result<StoredConfig, AdminError> {
        let _guard = self.lock.lock().await;
        let raw = fs::read_to_string(&self.path)?;
        self.ensure_revision(&raw, expected_revision)?;
        let mut document = raw.parse::<DocumentMut>()?;
        if document.get("bots").is_none() {
            document["bots"] = Item::ArrayOfTables(ArrayOfTables::new());
        }
        let bots = document["bots"].as_array_of_tables_mut().ok_or_else(|| {
            AdminError::BadRequest("the [[bots]] configuration is not an array of tables".into())
        })?;

        if let Some(existing_id) = existing_id {
            let table = bots
                .iter_mut()
                .find(|table| table.get("id").and_then(Item::as_str) == Some(existing_id))
                .ok_or_else(|| {
                    AdminError::NotFound(format!("bot '{}' was not found", existing_id))
                })?;
            apply_bot(table, update, false);
        } else {
            if bots
                .iter()
                .any(|table| table.get("id").and_then(Item::as_str) == Some(update.id.as_str()))
            {
                return Err(AdminError::Conflict(format!(
                    "bot '{}' already exists",
                    update.id
                )));
            }
            let mut table = Table::new();
            apply_bot(&mut table, update, true);
            bots.push(table);
        }

        self.save_document(raw, document)
    }

    pub async fn delete_bot(
        &self,
        bot_id: &str,
        expected_revision: &str,
    ) -> Result<StoredConfig, AdminError> {
        let _guard = self.lock.lock().await;
        let raw = fs::read_to_string(&self.path)?;
        self.ensure_revision(&raw, expected_revision)?;
        let mut document = raw.parse::<DocumentMut>()?;
        let bots = document["bots"].as_array_of_tables_mut().ok_or_else(|| {
            AdminError::BadRequest("the [[bots]] configuration is not an array of tables".into())
        })?;
        let index = bots
            .iter()
            .position(|table| table.get("id").and_then(Item::as_str) == Some(bot_id))
            .ok_or_else(|| AdminError::NotFound(format!("bot '{}' was not found", bot_id)))?;
        bots.remove(index);
        self.save_document(raw, document)
    }

    pub async fn revisions(&self) -> Result<Vec<RevisionView>, AdminError> {
        let _guard = self.lock.lock().await;
        let current_raw = fs::read_to_string(&self.path)?;
        let current_revision = revision_for(&current_raw);
        let current_metadata = fs::metadata(&self.path)?;
        let mut revisions = vec![RevisionView {
            revision: current_revision,
            created_at: modified_at(&current_metadata),
            size_bytes: current_metadata.len(),
            current: true,
        }];

        if self.revisions_dir.exists() {
            for entry in fs::read_dir(&self.revisions_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("toml") {
                    continue;
                }
                let raw = fs::read_to_string(&path)?;
                let metadata = entry.metadata()?;
                revisions.push(RevisionView {
                    revision: revision_for(&raw),
                    created_at: modified_at(&metadata),
                    size_bytes: metadata.len(),
                    current: false,
                });
            }
        }
        revisions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        revisions.dedup_by(|left, right| left.revision == right.revision);
        Ok(revisions)
    }

    pub async fn rollback(&self, revision: &str) -> Result<StoredConfig, AdminError> {
        let _guard = self.lock.lock().await;
        if !revision
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        {
            return Err(AdminError::BadRequest(
                "invalid revision identifier".to_string(),
            ));
        }
        let current_raw = fs::read_to_string(&self.path)?;
        if revision_for(&current_raw) == revision {
            return self.read_unlocked();
        }
        let mut selected = None;
        if self.revisions_dir.exists() {
            for entry in fs::read_dir(&self.revisions_dir)? {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("toml") {
                    continue;
                }
                let raw = fs::read_to_string(&path)?;
                if revision_for(&raw) == revision {
                    selected = Some(raw);
                    break;
                }
            }
        }
        let selected = selected.ok_or_else(|| {
            AdminError::NotFound(format!(
                "configuration revision '{}' was not found",
                revision
            ))
        })?;
        AppConfig::load_from_str(&selected)?;
        self.backup(&current_raw)?;
        atomic_replace(&self.path, &selected)?;
        self.read_unlocked()
    }

    fn read_unlocked(&self) -> Result<StoredConfig, AdminError> {
        let raw = fs::read_to_string(&self.path)?;
        let config = AppConfig::load_from_str(&raw)?;
        Ok(StoredConfig {
            config,
            revision: revision_for(&raw),
        })
    }

    fn ensure_revision(&self, raw: &str, expected: &str) -> Result<(), AdminError> {
        let current = revision_for(raw);
        if current != expected {
            return Err(AdminError::Conflict(format!(
                "configuration changed since it was opened; current revision is {}",
                current
            )));
        }
        Ok(())
    }

    fn save_document(
        &self,
        previous_raw: String,
        document: DocumentMut,
    ) -> Result<StoredConfig, AdminError> {
        let next_raw = document.to_string();
        let config = AppConfig::load_from_str(&next_raw)?;
        self.backup(&previous_raw)?;
        atomic_replace(&self.path, &next_raw)?;
        Ok(StoredConfig {
            config,
            revision: revision_for(&next_raw),
        })
    }

    fn backup(&self, raw: &str) -> Result<(), AdminError> {
        fs::create_dir_all(&self.revisions_dir)?;
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        let path = self
            .revisions_dir
            .join(format!("{}-{}.toml", timestamp, revision_for(raw)));
        if !path.exists() {
            fs::write(path, raw)?;
        }
        let mut entries: Vec<_> = fs::read_dir(&self.revisions_dir)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("toml")
            })
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        let remove_count = entries.len().saturating_sub(20);
        for entry in entries.into_iter().take(remove_count) {
            let _ = fs::remove_file(entry.path());
        }
        Ok(())
    }
}

fn apply_general(document: &mut DocumentMut, update: &GeneralMutation) {
    document["runtime"]["env"] = value(update.environment.trim());
    document["runtime"]["shutdown_timeout_secs"] = value(update.shutdown_timeout_secs as i64);
    document["runtime"]["task_grace_secs"] = value(update.task_grace_secs as i64);
    document["observability"]["level"] = value(update.log_level.trim());
    document["observability"]["json_logs"] = value(update.json_logs);

    document["admin_web"]["enabled"] = value(update.admin_enabled);
    document["admin_web"]["bind"] = value(update.admin_bind.trim());
    document["admin_web"]["log_capacity"] = value(update.log_capacity as i64);
    document["admin_web"]["audit_path"] = value(update.audit_path.trim());
    if let Some(token) = &update.admin_access_token {
        set_secret(&mut document["admin_web"], "access_token", token);
    }

    document["official_host"]["builtin_modules"] = string_array(&update.builtin_modules);
    document["official_host"]["plugin_modules"] = string_array(&update.plugin_modules);
    document["official_host"]["plugin_state_path"] = value(update.plugin_state_path.trim());
    document["official_host"]["plugin_bin_dir"] = value(update.plugin_bin_dir.trim());
    document["official_host"]["dynamic_plugin_timeout_secs"] =
        value(update.dynamic_plugin_timeout_secs as i64);
    document["official_host"]["proactive_send"]["queue_capacity"] =
        value(update.proactive_queue_capacity as i64);
    document["official_host"]["proactive_send"]["offline_ttl_secs"] =
        value(update.proactive_offline_ttl_secs as i64);
    document["official_host"]["webhook"]["enabled"] = value(update.webhook_enabled);
    document["official_host"]["webhook"]["bind"] = value(update.webhook_bind.trim());
    document["official_host"]["webhook"]["base_path"] = value(update.webhook_base_path.trim());
    document["official_host"]["webhook"]["max_body_bytes"] =
        value(update.webhook_max_body_bytes as i64);
    document["official_host"]["webhook"]["request_timeout_ms"] =
        value(update.webhook_request_timeout_ms as i64);
    document["official_host"]["webhook"]["max_in_flight"] =
        value(update.webhook_max_in_flight as i64);
    if let Some(token) = &update.webhook_access_token {
        set_secret(
            &mut document["official_host"]["webhook"],
            "access_token",
            token,
        );
    }
}

fn apply_bot(table: &mut Table, update: &BotMutation, new_bot: bool) {
    table["id"] = value(update.id.trim());
    set_optional_string(table, "account_id", update.account_id.as_deref());
    table["protocol"] = value(update.protocol.trim());
    table["transport"] = value(update.transport.trim());
    set_optional_string(table, "endpoint", update.endpoint.as_deref());
    set_optional_string(table, "bind", update.bind.as_deref());
    set_optional_string(table, "path", update.path.as_deref());
    set_optional_string(table, "appid", update.appid.as_deref());
    if let Some(token) = &update.access_token {
        set_secret_in_table(table, "access_token", token);
    } else if new_bot {
        table.remove("access_token");
    }
    if let Some(secret) = &update.secret {
        set_secret_in_table(table, "secret", secret);
    } else if new_bot {
        table.remove("secret");
    }
    table["intents"] = string_array(&update.intents);
    table["sandbox"] = value(update.sandbox);
    table["enabled"] = value(update.enabled);
    table["enabled_modules"] = string_array(&update.enabled_modules);
    table["owners"] = string_array(&update.owners);
    table["admins"] = string_array(&update.admins);
    table["auto_approve_friend_requests"] = value(update.auto_approve_friend_requests);
    table["auto_approve_group_invites"] = value(update.auto_approve_group_invites);
    table["auto_reply_poke_enabled"] = value(update.auto_reply_poke_enabled);
    set_optional_string(
        table,
        "auto_reply_poke_message",
        update.auto_reply_poke_message.as_deref(),
    );
    table["limiter"]["enable"] = value(update.limiter.enable);
    table["limiter"]["rate"] = value(update.limiter.rate);
    table["limiter"]["capacity"] = value(update.limiter.capacity as i64);
    table["limiter"]["timeout_secs"] = value(update.limiter.timeout_secs as i64);
}

fn set_optional_string(table: &mut Table, key: &str, value_to_set: Option<&str>) {
    match value_to_set
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value_to_set) => table[key] = value(value_to_set),
        None => {
            table.remove(key);
        }
    }
}

fn set_secret(item: &mut Item, key: &str, secret: &str) {
    if secret.is_empty() {
        item.as_table_mut().map(|table| table.remove(key));
    } else {
        item[key] = value(secret);
    }
}

fn set_secret_in_table(table: &mut Table, key: &str, secret: &str) {
    if secret.is_empty() {
        table.remove(key);
    } else {
        table[key] = value(secret);
    }
}

fn string_array(values: &[String]) -> Item {
    let mut array = Array::new();
    for value_to_add in values {
        let trimmed = value_to_add.trim();
        if !trimmed.is_empty() {
            array.push(trimmed);
        }
    }
    Item::Value(Value::Array(array))
}

fn revision_for(raw: &str) -> String {
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn modified_at(metadata: &fs::Metadata) -> String {
    metadata
        .modified()
        .ok()
        .map(chrono::DateTime::<Utc>::from)
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn atomic_replace(path: &Path, raw: &str) -> Result<(), AdminError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("qimen.toml");
    let temp = path.with_file_name(format!("{}.{}.tmp", file_name, std::process::id()));
    fs::write(&temp, raw)?;
    if let Err(first_error) = fs::rename(&temp, path) {
        if path.exists() {
            fs::remove_file(path)?;
            fs::rename(&temp, path)?;
        } else {
            return Err(AdminError::internal(first_error));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RateLimiterView;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn revisions_change_with_content() {
        assert_ne!(revision_for("a"), revision_for("b"));
        assert_eq!(revision_for("a"), revision_for("a"));
    }

    #[tokio::test]
    async fn bot_update_preserves_unsubmitted_secret_values() {
        let directory = temp_directory();
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("base.toml");
        fs::write(&path, test_config()).unwrap();
        let store = ConfigStore::new(&path);
        let current = store.read().await.unwrap();
        let update = BotMutation {
            id: "test-bot".to_string(),
            account_id: Some("10001".to_string()),
            protocol: "onebot11".to_string(),
            transport: "ws-forward".to_string(),
            endpoint: Some("ws://127.0.0.1:4001".to_string()),
            bind: None,
            path: None,
            access_token: None,
            appid: None,
            secret: None,
            intents: Vec::new(),
            sandbox: false,
            enabled: true,
            enabled_modules: vec!["command".to_string()],
            owners: Vec::new(),
            admins: Vec::new(),
            auto_approve_friend_requests: false,
            auto_approve_group_invites: false,
            auto_reply_poke_enabled: false,
            auto_reply_poke_message: None,
            limiter: RateLimiterView {
                enable: true,
                rate: 4.0,
                capacity: 8,
                timeout_secs: 1,
            },
        };

        let saved = store
            .save_bot(Some("test-bot"), &current.revision, &update)
            .await
            .unwrap();
        assert_ne!(saved.revision, current.revision);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(r#"access_token = "${QIMEN_TEST_TOKEN}""#));
        assert!(raw.contains("ws://127.0.0.1:4001"));
        assert!(saved.config.bots[0].limiter.enable);
        assert_eq!(saved.config.bots[0].limiter.capacity, 8);
        fs::remove_dir_all(directory).unwrap();
    }

    fn temp_directory() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "qimen-admin-web-test-{}-{}",
            std::process::id(),
            nonce
        ))
    }

    fn test_config() -> &'static str {
        r#"
[runtime]
env = "test"
shutdown_timeout_secs = 5
task_grace_secs = 2

[observability]
level = "info"
json_logs = false
metrics_bind = "127.0.0.1:0"

[admin_web]
enabled = false
bind = "127.0.0.1:3210"
log_capacity = 100
audit_path = "audit.jsonl"

[official_host]
builtin_modules = []
plugin_modules = []
plugin_state_path = "plugin-state.toml"
plugin_bin_dir = "plugins"
dynamic_plugin_timeout_secs = 5

[official_host.proactive_send]
queue_capacity = 16
offline_ttl_secs = 0

[official_host.webhook]
enabled = false
bind = "127.0.0.1:0"
base_path = "/webhooks"
max_body_bytes = 1024
request_timeout_ms = 1000
max_in_flight = 4
access_token = ""

[[bots]]
id = "test-bot"
protocol = "onebot11"
transport = "ws-forward"
endpoint = "ws://127.0.0.1:3001"
access_token = "${QIMEN_TEST_TOKEN}"
enabled = true
"#
    }
}
