//! MCP over streamable HTTP, mounted at `/mcp`. The tools are `ops::tools()`; the dashboard is an MCP Apps `ui://` resource.
use crate::{AppState, frontend, ops};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::*,
    service::RequestContext,
    transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager},
};
use serde_json::{Value, json};

const UI_URI: &str = "ui://spec11/dashboard";
const UI_MIME: &str = "text/html;profile=mcp-app";

#[derive(Clone)]
struct Mcp(AppState);

impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_resources().build()).with_instructions(format!("Deterministic spec-driven-development evidence for one repo. {}", ops::evidence_note()))
    }

    async fn list_tools(&self, _: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>) -> Result<ListToolsResult, ErrorData> {
        let tools = ops::tools()
            .into_iter()
            .map(|t| {
                let schema = t.schema.as_object().cloned().unwrap_or_default();
                let tool = Tool::new(t.name, t.description, schema);
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
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        }
        .into())
    }

    async fn list_resources(&self, _: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(vec![Resource::new(UI_URI, "spec11 dashboard").with_description("Spec-driven status dashboard (MCP Apps)").with_mime_type(UI_MIME)]))
    }

    async fn read_resource(&self, request: ReadResourceRequestParams, _: RequestContext<RoleServer>) -> Result<ReadResourceResponse, ErrorData> {
        if request.uri != UI_URI {
            return Err(ErrorData::resource_not_found(format!("unknown resource {}", request.uri), None));
        }
        // debug builds without a web build: point at the dev URL instead of failing
        let html = frontend::index_html().unwrap_or_else(|| format!("<!doctype html><meta charset=utf-8><p>Dashboard not built. Open <a href=\"{0}\">{0}</a></p>", self.0.dashboard));
        Ok(ReadResourceResult::new(vec![ResourceContents::text(html, UI_URI).with_mime_type(UI_MIME)]).into())
    }
}

pub fn service(state: AppState, hosts: &[String]) -> StreamableHttpService<impl ServerHandler, LocalSessionManager> {
    let origins: Vec<String> = hosts.iter().map(|h| format!("http://{h}")).collect();
    let config = StreamableHttpServerConfig::default().with_allowed_hosts(hosts.to_vec()).with_allowed_origins(origins).with_legacy_session_mode(false).with_json_response(true);
    StreamableHttpService::new(move || Ok(Mcp(state.clone())), Default::default(), config)
}
