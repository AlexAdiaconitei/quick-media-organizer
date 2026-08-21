use std::process::Command;

/// Resolves the repository this build comes from, so the app can link to it
/// without anyone hardcoding a fork's URL: GITHUB_REPOSITORY in CI, the git
/// "origin" remote otherwise.
fn repository_url() -> Option<String> {
    if let Ok(slug) = std::env::var("GITHUB_REPOSITORY") {
        if !slug.trim().is_empty() {
            return Some(format!("https://github.com/{}", slug.trim()));
        }
    }

    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let remote = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let remote = remote.strip_suffix(".git").unwrap_or(&remote).to_string();

    // git@github.com:owner/repo -> https://github.com/owner/repo
    if let Some(path) = remote.strip_prefix("git@github.com:") {
        return Some(format!("https://github.com/{path}"));
    }
    remote.starts_with("http").then_some(remote)
}

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_REPOSITORY");
    if let Some(url) = repository_url() {
        println!("cargo:rustc-env=QMO_REPOSITORY_URL={url}");
    }
    tauri_build::build()
}
