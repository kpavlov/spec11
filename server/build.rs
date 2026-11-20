// Release builds embed web/dist, so build it first. SKIP_WEB_BUILD=1 when dist is already built (CI, `make build`).
fn main() {
    // (NFR-08) Only the dev release recipe supplies build provenance.
    println!("cargo:rerun-if-env-changed=SPEC17_DEV_HASH");
    println!("cargo:rerun-if-env-changed=SPEC17_DEV_DATE");
    let mut version = std::env::var("CARGO_PKG_VERSION").unwrap();
    match (std::env::var("SPEC17_DEV_HASH").ok(), std::env::var("SPEC17_DEV_DATE").ok()) {
        (Some(hash), Some(date)) => {
            assert!(!hash.is_empty() && hash.bytes().all(|b| b.is_ascii_hexdigit()), "invalid dev commit hash");
            assert!(date.len() == 20 && date.bytes().all(|b| b.is_ascii_digit() || b"-:TZ".contains(&b)), "invalid dev build date");
            version.push_str(&format!("-dev+{hash} ({date})"));
        }
        (None, None) => {}
        _ => panic!("set both SPEC17_DEV_HASH and SPEC17_DEV_DATE for a dev release"),
    }
    println!("cargo:rustc-env=SPEC17_VERSION={version}");

    println!("cargo:rerun-if-changed=../web/src");
    println!("cargo:rerun-if-changed=../web/index.html");
    println!("cargo:rerun-if-changed=../web/package.json");
    println!("cargo:rerun-if-env-changed=SKIP_WEB_BUILD");
    if std::env::var_os("SKIP_WEB_BUILD").is_some() {
        println!("cargo:rerun-if-changed=../web/dist/index.html");
    } else if std::env::var("PROFILE").is_ok_and(|p| p == "release") {
        let ok = std::process::Command::new("pnpm").args(["run", "build"]).current_dir("../web").status().expect("pnpm not found: install Node and pnpm or set SKIP_WEB_BUILD=1").success();
        assert!(ok, "web build failed");
    }
}
