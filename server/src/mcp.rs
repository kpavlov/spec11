//! MCP over streamable HTTP, mounted at `/mcp`. The tools are `ops::tools()`; the dashboard is an MCP Apps `ui://` resource.
use crate::{AppState, frontend, ops};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::*,
    service::{RequestContext, SubscriptionContext},
    transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager},
};
use serde_json::{Value, json};

/// How often a subscription looks for a change.
const POLL: std::time::Duration = std::time::Duration::from_secs(1);
const UI_URI: &str = "ui://spec11/dashboard";
use ops::{JSON_MIME, SCHEMA_MIME, TEXT_MIME};
const UI_MIME: &str = "text/html;profile=mcp-app";

#[derive(Clone)]
struct Mcp(AppState);

impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_resources().enable_resources_subscribe().build()).with_instructions(format!("Deterministic spec-driven-development evidence for one repo. {}", ops::NOTE))
    }

    async fn list_tools(&self, _: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>) -> Result<ListToolsResult, ErrorData> {
        let tools = ops::tools()
            .iter()
            .map(|t| {
                let schema = t.schema.as_object().cloned().unwrap_or_default();
                let mut tool = Tool::new(t.name, t.description, schema);
                if let Some(Value::Object(out)) = ops::output_schema(t.name) {
                    tool = tool.with_raw_output_schema(std::sync::Arc::new(out));
                }
                if t.ui {
                    let mut meta = MetaObject::new();
                    meta.insert("ui".into(), json!({"resourceUri": UI_URI}));
                    tool.with_meta(meta)
                } else {
                    tool
                }
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(&self, request: CallToolRequestParams, _: RequestContext<RoleServer>) -> Result<CallToolResponse, ErrorData> {
        let args = Value::Object(request.arguments.unwrap_or_default());
        let (root, dash, name, port) = (self.0.root.clone(), self.0.dashboard.clone(), request.name.to_string(), self.0.mcp_port);
        let r = tokio::task::spawn_blocking(move || ops::call(root.as_deref(), &name, &args, &dash, port)).await.map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(match r {
            Ok((text, data)) => {
                let mut res = CallToolResult::success(vec![ContentBlock::text(text)]);
                res.structured_content = Some(data);
                res
            }
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_string())]),
        }
        .into())
    }

    async fn list_resources(&self, _: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>) -> Result<ListResourcesResult, ErrorData> {
        // a JSON resource says which schema types it: in its description and in `_meta.schema`
        let json = |uri: String, name: &str, about: &str| {
            let mut r = Resource::new(&uri, name).with_mime_type(JSON_MIME);
            match ops::schema_uri(&uri) {
                Some(schema) => {
                    let mut meta = MetaObject::new();
                    meta.insert("schema".into(), json!(schema));
                    r = r.with_description(format!("{about} Typed by the JSON Schema at {schema}.")).with_meta(meta);
                }
                None => r = r.with_description(about),
            }
            r
        };
        let mut all = vec![Resource::new(UI_URI, "spec11 dashboard").with_description("Spec-driven status dashboard (MCP Apps)").with_mime_type(UI_MIME)];
        for tool in ops::VIEW_TOOLS {
            all.push(Resource::new(format!("spec11://{tool}"), format!("spec11 {tool}")).with_description(ops::tool_description(tool)).with_mime_type(TEXT_MIME));
        }
        for tool in ops::VIEW_TOOLS {
            all.push(json(format!("spec11://{tool}.json"), &format!("spec11 {tool} (JSON)"), ops::tool_description(tool)));
        }
        all.extend(ops::RESOURCES.iter().map(|(uri, name, about, _)| json(uri.to_string(), name, about)));
        all.extend(ops::SCHEMAS.iter().map(|n| {
            let uri = format!("{}{n}", ops::SCHEMA_PREFIX);
            Resource::new(uri, format!("spec11 {n} schema")).with_description(format!("JSON Schema of the `{n}` result: the JSON resource spec11://{n}(.json) and the structuredContent of its tool.")).with_mime_type(SCHEMA_MIME)
        }));
        Ok(ListResourcesResult::with_all_items(all))
    }

    async fn read_resource(&self, request: ReadResourceRequestParams, _: RequestContext<RoleServer>) -> Result<ReadResourceResponse, ErrorData> {
        if request.uri != UI_URI {
            let (root, uri) = (self.0.root.clone(), request.uri.to_string());
            let text = tokio::task::spawn_blocking(move || ops::read_resource(root.as_deref(), &uri)).await.map_err(|e| ErrorData::internal_error(e.to_string(), None))?.map_err(|e| ErrorData::internal_error(e, None))?;
            return match text {
                Some((mime, t)) => Ok(ReadResourceResult::new(vec![ResourceContents::text(t, request.uri.as_str()).with_mime_type(mime)]).into()),
                None => Err(ErrorData::resource_not_found(format!("unknown resource {}", request.uri), None)),
            };
        }
        // debug builds without a web build: point at the dev URL instead of failing
        let html = frontend::index_html().unwrap_or_else(|| format!("<!doctype html><meta charset=utf-8><p>Dashboard not built. Open <a href=\"{0}\">{0}</a></p>", self.0.dashboard));
        Ok(ReadResourceResult::new(vec![ResourceContents::text(html, UI_URI).with_mime_type(UI_MIME)]).into())
    }

    // (API-08)
    /// Only the resources this server has can be subscribed to; list-changed is never offered (the lists are static).
    fn accepted_subscription_filter(&self, requested: &SubscriptionFilter) -> Option<SubscriptionFilter> {
        let mut f = SubscriptionFilter::new();
        f.resource_subscriptions = requested.resource_subscriptions.as_ref().map(|v| v.iter().filter(|u| ops::resource_uris().any(|k| k == **u)).cloned().collect());
        Some(f)
    }

    // (API-08)
    /// Polls `ops::stamp` and sends `notifications/resources/updated` for every accepted URI when the repo changed.
    /// Ends when the client cancels or the server shuts down (a graceful `subscriptions/listen` result).
    async fn listen(&self, context: SubscriptionContext) -> Result<(), ErrorData> {
        let stamp = || {
            let root = self.0.root.clone();
            async move { tokio::task::spawn_blocking(move || ops::stamp(root.as_deref())).await.unwrap_or(0) }
        };
        let uris = context.accepted().resource_subscriptions.clone().unwrap_or_default();
        if uris.is_empty() {
            return Ok(()); // nothing to tell: no point scanning the repo
        }
        let mut last = stamp().await;
        let mut tick = tokio::time::interval(POLL);
        loop {
            tokio::select! {
                _ = context.cancelled() => return Ok(()),
                _ = self.0.shutdown.cancelled() => return Ok(()),
                _ = tick.tick() => {
                    let now = stamp().await;
                    if now == last {
                        continue;
                    }
                    last = now;
                    for u in &uris {
                        if context.sink().notify_resource_updated(u.as_str()).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

pub fn service(state: AppState, hosts: &[String]) -> StreamableHttpService<impl ServerHandler, LocalSessionManager> {
    let origins: Vec<String> = hosts.iter().map(|h| format!("http://{h}")).collect();
    // no rmcp cancellation token on purpose: it would drop open streams before `listen` can send its graceful result
    let config = StreamableHttpServerConfig::default().with_allowed_hosts(hosts.to_vec()).with_allowed_origins(origins).with_legacy_session_mode(false).with_json_response(true);
    StreamableHttpService::new(move || Ok(Mcp(state.clone())), Default::default(), config)
}
