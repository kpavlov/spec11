// Release builds embed web/dist, so build it first. SKIP_WEB_BUILD=1 when dist is already built (CI, `make build`).
fn main() {
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
