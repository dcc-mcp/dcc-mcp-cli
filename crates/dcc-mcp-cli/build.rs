fn main() {
    let manifest = "../../Cargo.toml";
    println!("cargo:rerun-if-changed={manifest}");
    let workspace: toml::Table = std::fs::read_to_string(manifest)
        .expect("workspace manifest")
        .parse()
        .expect("valid TOML");
    let tag = workspace["workspace"]["dependencies"]["dcc-mcp-client"]["tag"]
        .as_str()
        .expect("pinned Core SDK tag");
    println!(
        "cargo:rustc-env=DCC_MCP_BUNDLED_CORE_VERSION={}",
        tag.strip_prefix('v').expect("versioned Core tag")
    );
    println!(
        "cargo:rustc-env=DCC_MCP_BUILD_TARGET={}",
        std::env::var("TARGET").expect("Cargo target")
    );
}
