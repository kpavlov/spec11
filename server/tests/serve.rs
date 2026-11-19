use serde_json::{Value, json};
use std::{path::Path, process::{Command, Stdio}, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[doc = "[API-22]"]
#[doc = "[API-23]"]
#[tokio::test]
async fn independent_stdio_sessions_use_the_same_tools_and_exit_on_eof() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/test-data/nospecs");
    let session = |serve: bool| {
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_spec11"));
        command.arg("--root").arg(&root);
        if serve {
            command.arg("serve");
        }
        command.args(["--stdio-mcp", "--no-http-mcp", "--no-ui"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        async move {
            let mut child = command.spawn().unwrap();
            let mut stdin = child.stdin.take().unwrap();
            let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
            let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}
            }});
            stdin.write_all(format!("{init}\n").as_bytes()).await.unwrap();
            let response: Value = serde_json::from_str(&stdout.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(response["result"]["protocolVersion"], "2025-03-26");
            let notification = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
            let report = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"report","arguments":{}}});
            stdin.write_all(format!("{notification}\n{report}\n").as_bytes()).await.unwrap();
            let response: Value = serde_json::from_str(&stdout.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(response["id"], 2);
            assert!(response["result"]["structuredContent"].is_object(), "{response}");
            drop(stdin);
            assert!(stdout.next_line().await.unwrap().is_none());
            let output = child.wait_with_output().await.unwrap();
            let diagnostics = String::from_utf8(output.stderr).unwrap();
            assert!(output.status.success(), "{diagnostics}");
            assert!(diagnostics.is_empty(), "{diagnostics}");
        }
    };
    tokio::time::timeout(Duration::from_secs(10), async { tokio::join!(session(false), session(true)); }).await.unwrap();
}

#[doc = "[SETUP-13]"]
#[test]
fn cli_install_defaults_to_http_in_both_scopes() {
    let home = std::env::temp_dir().join(format!("spec11-install-home-{}", std::process::id()));
    let root = home.join("repo");
    std::fs::create_dir_all(root.join("specs")).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_spec11")).env("HOME", &home)
            .arg("--root").arg(&root).arg("install").args(args).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    };
    run(&["--port", "7421"]);
    run(&["--global", "--port", "7422"]);
    for (scope, port) in [(&root, 7421), (&home, 7422)] {
        let config: Value = serde_json::from_slice(&std::fs::read(scope.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["spec11"], json!({"type":"http", "url":format!("http://127.0.0.1:{port}/mcp")}));
        assert!(scope.join(".agents/skills/spec11/SKILL.md").is_file());
    }
    std::fs::remove_dir_all(home).unwrap();
}

#[doc = "[API-23]"]
#[test]
fn stdio_failure_is_silent() {
    let missing = std::env::temp_dir().join(format!("spec11-no-repo-{}", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_spec11")).arg("--root").arg(&missing).args(["--stdio-mcp", "--no-http-mcp", "--no-ui"]).stdin(Stdio::null()).output().unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
}
