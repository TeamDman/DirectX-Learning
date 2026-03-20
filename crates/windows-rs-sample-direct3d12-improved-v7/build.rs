use std::process::Command;

fn main() {
    add_exe_resources();
    add_git_revision();
}

fn add_exe_resources() {
    println!("cargo:rerun-if-changed=resources");

    embed_resource::compile("resources/app.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed resources");
}

fn add_git_revision() {
    let rev = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| output.status.success().then_some(output.stdout))
        .and_then(|stdout| String::from_utf8(stdout).ok())
        .map_or_else(|| "unknown".to_string(), |stdout| stdout.trim().to_string());

    println!("cargo:rustc-env=GIT_REVISION={rev}");
}
