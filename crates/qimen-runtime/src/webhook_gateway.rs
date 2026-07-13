use crate::dynamic_runtime::{DynamicPluginRuntime, OwnedWebhookResponse};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::response::Response;
use axum::routing::any;
use qimen_config::WebhookGatewayConfig;
use qimen_error::{QimenError, Result};
use qimen_host_types::{DynamicPluginReportEntry, DynamicWebhookDescriptor};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub(crate) struct WebhookGateway {
    inner: Arc<WebhookGatewayInner>,
}

struct WebhookGatewayInner {
    config: WebhookGatewayConfig,
    dynamic_runtime: Arc<Mutex<DynamicPluginRuntime>>,
    registry: RwLock<RouteRegistry>,
    activity: Arc<ActiveRequests>,
    semaphore: Arc<Semaphore>,
}

#[derive(Default)]
struct RouteRegistry {
    accepting: bool,
    routes: HashMap<RouteKey, DynamicWebhookDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RouteKey {
    method: Method,
    path: String,
}

#[derive(Default)]
struct ActiveRequests {
    count: Mutex<usize>,
    idle: Condvar,
}

struct ActiveRequestGuard {
    activity: Arc<ActiveRequests>,
}

impl Drop for ActiveRequestGuard {
    fn drop(&mut self) {
        if let Ok(mut count) = self.activity.count.lock() {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.activity.idle.notify_all();
            }
        }
    }
}

enum RouteLookup {
    Ready(DynamicWebhookDescriptor, ActiveRequestGuard),
    NotFound,
    Unavailable,
}

impl WebhookGateway {
    pub(crate) fn new(
        config: WebhookGatewayConfig,
        dynamic_runtime: Arc<Mutex<DynamicPluginRuntime>>,
    ) -> Self {
        let max_in_flight = config.max_in_flight;
        Self {
            inner: Arc::new(WebhookGatewayInner {
                config,
                dynamic_runtime,
                registry: RwLock::new(RouteRegistry::default()),
                activity: Arc::new(ActiveRequests::default()),
                semaphore: Arc::new(Semaphore::new(max_in_flight)),
            }),
        }
    }

    pub(crate) fn install_entries(&self, entries: &[DynamicPluginReportEntry]) -> Result<usize> {
        let routes = build_route_table(&self.inner.config.base_path, entries)?;
        let count = routes.len();
        let mut registry =
            self.inner.registry.write().map_err(|_| {
                QimenError::Runtime("webhook route registry lock poisoned".to_string())
            })?;
        registry.routes = routes;
        registry.accepting = true;
        Ok(count)
    }

    /// Stop admitting requests and wait until every blocking plugin callback returns.
    pub(crate) fn pause_and_wait(&self) {
        if let Ok(mut registry) = self.inner.registry.write() {
            registry.accepting = false;
        }
        let Ok(mut count) = self.inner.activity.count.lock() else {
            return;
        };
        while *count != 0 {
            match self.inner.activity.idle.wait(count) {
                Ok(next) => count = next,
                Err(_) => return,
            }
        }
    }

    pub(crate) async fn serve(&self) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(&self.inner.config.bind)
            .await
            .map_err(|err| {
                QimenError::Transport(format!(
                    "failed to bind webhook gateway at '{}': {err}",
                    self.inner.config.bind
                ))
            })?;
        tracing::info!(
            address = %self.inner.config.bind,
            base_path = %self.inner.config.base_path,
            "webhook gateway listening"
        );
        let app = Router::new()
            .fallback(any(dispatch_webhook))
            .with_state(self.clone());
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|err| QimenError::Transport(format!("webhook gateway failed: {err}")))
    }

    fn acquire_route(&self, method: &Method, path: &str) -> RouteLookup {
        let Ok(registry) = self.inner.registry.read() else {
            return RouteLookup::Unavailable;
        };
        if !registry.accepting {
            return RouteLookup::Unavailable;
        }
        let key = RouteKey {
            method: method.clone(),
            path: path.to_string(),
        };
        let Some(descriptor) = registry.routes.get(&key).cloned() else {
            return RouteLookup::NotFound;
        };
        let Ok(mut count) = self.inner.activity.count.lock() else {
            return RouteLookup::Unavailable;
        };
        *count += 1;
        RouteLookup::Ready(
            descriptor,
            ActiveRequestGuard {
                activity: Arc::clone(&self.inner.activity),
            },
        )
    }
}

async fn dispatch_webhook(
    State(gateway): State<WebhookGateway>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
) -> Response<Body> {
    if !authorized(request.headers(), &gateway.inner.config.access_token) {
        return plain_response(StatusCode::UNAUTHORIZED, "unauthorized");
    }

    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().unwrap_or_default().to_string();
    let (descriptor, active_guard) = match gateway.acquire_route(&method, &path) {
        RouteLookup::Ready(descriptor, guard) => (descriptor, guard),
        RouteLookup::NotFound => return plain_response(StatusCode::NOT_FOUND, "not found"),
        RouteLookup::Unavailable => {
            return plain_response(StatusCode::SERVICE_UNAVAILABLE, "webhook gateway reloading");
        }
    };

    let permit = match Arc::clone(&gateway.inner.semaphore).try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return plain_response(StatusCode::TOO_MANY_REQUESTS, "too many requests"),
    };
    let headers_json = serialize_headers(request.headers());
    let body = match to_bytes(request.into_body(), gateway.inner.config.max_body_bytes).await {
        Ok(body) => body.to_vec(),
        Err(_) => return plain_response(StatusCode::PAYLOAD_TOO_LARGE, "payload too large"),
    };

    let handle = {
        let mut runtime = match gateway.inner.dynamic_runtime.lock() {
            Ok(runtime) => runtime,
            Err(_) => {
                return plain_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "plugin runtime unavailable",
                );
            }
        };
        match runtime.get_library(&descriptor.library_path) {
            Ok(handle) => handle,
            Err(err) => {
                tracing::error!(
                    plugin = %descriptor.plugin_id,
                    error = %err,
                    "failed to load webhook plugin"
                );
                return plain_response(StatusCode::SERVICE_UNAVAILABLE, "plugin unavailable");
            }
        }
    };

    let callback_descriptor = descriptor.clone();
    let callback = tokio::task::spawn_blocking(move || {
        let _active_guard = active_guard;
        let _permit = permit;
        DynamicPluginRuntime::execute_webhook_on_handle(
            &handle,
            &callback_descriptor,
            method.as_str(),
            &path,
            &query,
            &headers_json,
            body,
            &remote_addr.to_string(),
        )
    });
    let timeout = Duration::from_millis(gateway.inner.config.request_timeout_ms);
    match tokio::time::timeout(timeout, callback).await {
        Ok(Ok(Ok(response))) => render_plugin_response(&descriptor, response),
        Ok(Ok(Err(err))) => {
            tracing::warn!(plugin = %descriptor.plugin_id, error = %err, "webhook callback failed");
            plain_response(StatusCode::INTERNAL_SERVER_ERROR, "webhook callback failed")
        }
        Ok(Err(err)) => {
            tracing::warn!(plugin = %descriptor.plugin_id, error = %err, "webhook task panicked");
            plain_response(StatusCode::INTERNAL_SERVER_ERROR, "webhook callback failed")
        }
        Err(_) => {
            if let Ok(mut runtime) = gateway.inner.dynamic_runtime.lock() {
                runtime.record_timeout(&descriptor.library_path);
            }
            tracing::warn!(
                plugin = %descriptor.plugin_id,
                timeout_ms = gateway.inner.config.request_timeout_ms,
                "webhook callback timed out; callback remains isolated until it returns"
            );
            plain_response(StatusCode::GATEWAY_TIMEOUT, "webhook callback timed out")
        }
    }
}

fn render_plugin_response(
    descriptor: &DynamicWebhookDescriptor,
    response: OwnedWebhookResponse,
) -> Response<Body> {
    if !response.queued_sends.is_empty() {
        tracing::warn!(
            plugin = %descriptor.plugin_id,
            count = response.queued_sends.len(),
            "discarding legacy queued sends from webhook callback; use BotApi::for_account(...) or BotApi::for_bot(...) with a real-time send method"
        );
    }
    let status = StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut output = Response::new(Body::from(response.body));
    *output.status_mut() = status;
    apply_response_headers(
        output.headers_mut(),
        &response.headers_json,
        &descriptor.plugin_id,
    );
    output
}

fn build_route_table(
    base_path: &str,
    entries: &[DynamicPluginReportEntry],
) -> Result<HashMap<RouteKey, DynamicWebhookDescriptor>> {
    let mut routes = HashMap::new();
    for plugin in entries {
        if plugin.webhooks.is_empty() {
            continue;
        }
        validate_plugin_segment(&plugin.plugin_id)?;
        for webhook in &plugin.webhooks {
            let method = Method::from_bytes(webhook.method.as_bytes()).map_err(|_| {
                QimenError::Config(format!(
                    "plugin '{}' declares invalid webhook method '{}'",
                    plugin.plugin_id, webhook.method
                ))
            })?;
            validate_local_path(&plugin.plugin_id, &webhook.path)?;
            if webhook.callback_symbol.trim().is_empty() {
                return Err(QimenError::Config(format!(
                    "plugin '{}' declares a webhook with an empty callback symbol",
                    plugin.plugin_id
                )));
            }
            let base_path = base_path.trim_end_matches('/');
            let full_path = format!("{base_path}/{}{}", plugin.plugin_id, webhook.path);
            let key = RouteKey {
                method: method.clone(),
                path: full_path.clone(),
            };
            let descriptor = DynamicWebhookDescriptor {
                plugin_id: plugin.plugin_id.clone(),
                library_path: plugin.path.clone(),
                method: method.to_string(),
                path: full_path,
                callback_symbol: webhook.callback_symbol.clone(),
            };
            if let Some(existing) = routes.insert(key, descriptor) {
                return Err(QimenError::Config(format!(
                    "webhook route conflict between plugins '{}' and '{}'",
                    existing.plugin_id, plugin.plugin_id
                )));
            }
        }
    }
    Ok(routes)
}

fn validate_plugin_segment(plugin_id: &str) -> Result<()> {
    if plugin_id.is_empty()
        || matches!(plugin_id, "." | "..")
        || !plugin_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(QimenError::Config(format!(
            "plugin id '{}' cannot be used in a webhook URL",
            plugin_id
        )));
    }
    Ok(())
}

fn validate_local_path(plugin_id: &str, path: &str) -> Result<()> {
    if !path.starts_with('/')
        || path.contains('?')
        || path.contains('#')
        || path.contains('*')
        || path.contains("//")
        || path.split('/').any(|segment| matches!(segment, "." | ".."))
    {
        return Err(QimenError::Config(format!(
            "plugin '{}' declares invalid exact webhook path '{}'",
            plugin_id, path
        )));
    }
    Ok(())
}

fn authorized(headers: &HeaderMap, access_token: &str) -> bool {
    if access_token.is_empty() {
        return true;
    }
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == format!("Bearer {access_token}"))
}

fn serialize_headers(headers: &HeaderMap) -> String {
    let mut output = Map::new();
    for name in headers.keys() {
        let values: Vec<Value> = headers
            .get_all(name)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(|value| Value::String(value.to_string()))
            .collect();
        if values.len() == 1 {
            output.insert(name.to_string(), values[0].clone());
        } else if !values.is_empty() {
            output.insert(name.to_string(), Value::Array(values));
        }
    }
    Value::Object(output).to_string()
}

fn apply_response_headers(headers: &mut HeaderMap, headers_json: &str, plugin_id: &str) {
    let Ok(Value::Object(values)) = serde_json::from_str::<Value>(headers_json) else {
        if !headers_json.trim().is_empty() && headers_json.trim() != "{}" {
            tracing::warn!(plugin = %plugin_id, "ignoring invalid webhook response headers JSON");
        }
        return;
    };
    for (name, value) in values {
        if is_hop_by_hop_header(&name) {
            continue;
        }
        let Some(value) = value.as_str() else {
            continue;
        };
        let Ok(name) = HeaderName::try_from(name) else {
            continue;
        };
        let Ok(value) = HeaderValue::try_from(value) else {
            continue;
        };
        headers.append(name, value);
    }
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
    )
}

fn plain_response(status: StatusCode, body: &'static str) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use qimen_host_types::DynamicWebhookEntry;

    fn plugin(id: &str, method: &str, path: &str) -> DynamicPluginReportEntry {
        DynamicPluginReportEntry {
            path: format!("plugins/bin/{id}.so"),
            plugin_id: id.to_string(),
            plugin_version: "0.1.0".to_string(),
            api_version: "0.5".to_string(),
            commands: Vec::new(),
            routes: Vec::new(),
            interceptors: Vec::new(),
            webhooks: vec![DynamicWebhookEntry {
                method: method.to_string(),
                path: path.to_string(),
                callback_symbol: "handle_webhook".to_string(),
            }],
            command_name: String::new(),
            command_description: String::new(),
            callback_symbol: String::new(),
            notice_route: String::new(),
            notice_callback_symbol: String::new(),
            request_route: String::new(),
            request_callback_symbol: String::new(),
            meta_route: String::new(),
            meta_callback_symbol: String::new(),
        }
    }

    #[test]
    fn routes_are_namespaced_by_plugin() {
        let routes = build_route_table(
            "/webhooks",
            &[
                plugin("alpha", "POST", "/events"),
                plugin("beta", "POST", "/events"),
            ],
        )
        .unwrap();
        assert!(routes.contains_key(&RouteKey {
            method: Method::POST,
            path: "/webhooks/alpha/events".to_string(),
        }));
        assert!(routes.contains_key(&RouteKey {
            method: Method::POST,
            path: "/webhooks/beta/events".to_string(),
        }));
    }

    #[test]
    fn duplicate_exact_route_is_rejected() {
        let error = build_route_table(
            "/webhooks",
            &[
                plugin("alpha", "POST", "/events"),
                plugin("alpha", "POST", "/events"),
            ],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("route conflict"));
    }

    #[test]
    fn traversal_and_wildcards_are_rejected() {
        assert!(build_route_table("/webhooks", &[plugin("alpha", "POST", "/../events")]).is_err());
        assert!(build_route_table("/webhooks", &[plugin("alpha", "POST", "/./events")]).is_err());
        assert!(build_route_table("/webhooks", &[plugin("alpha", "POST", "/events/*")]).is_err());
        assert!(build_route_table("/webhooks", &[plugin("..", "POST", "/events")]).is_err());
    }

    #[test]
    fn root_base_path_does_not_create_a_double_slash() {
        let routes = build_route_table("/", &[plugin("alpha", "POST", "/events")]).unwrap();
        assert!(routes.contains_key(&RouteKey {
            method: Method::POST,
            path: "/alpha/events".to_string(),
        }));
    }
}
