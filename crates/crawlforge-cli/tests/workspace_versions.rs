//! The version is written in more than one place, and a release that bumps only one of them
//! publishes something broken.
//!
//! `crawlforge-core` and `crawlforge-rules` are published on crates.io, so the workspace names
//! them with a `version` besides the `path`: inside the repository cargo uses the path, in the
//! published package only the version remains. If `[workspace.package] version` moves to 0.22.0
//! and those stay at 0.21.2, `crawlforge 0.22.0` on crates.io depends on the old engine — or, at
//! the first incompatible change, does not build at all. `package.json` carries it too, because the
//! web installs this repository by tag and reads its version from there.
//!
//! The test reads the files of the repository, so it only runs inside it: from a package
//! downloaded from crates.io there is no workspace to check, and it says so instead of failing.

use std::path::Path;

/// The version of this crate, which inherits `[workspace.package] version`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn repo_file(name: &str) -> Option<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(name);
    std::fs::read_to_string(path).ok()
}

/// The line of `[workspace.dependencies]` that declares `name`, if any.
fn dependency_line<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name} = {{");
    manifest.lines().find(|l| l.trim_start().starts_with(&prefix))
}

#[test]
fn internal_dependencies_follow_the_workspace_version() {
    let Some(manifest) = repo_file("Cargo.toml").filter(|m| m.contains("[workspace.dependencies]"))
    else {
        eprintln!("not inside the CrawlForge workspace: nothing to check");
        return;
    };
    let expected = format!("version = \"{VERSION}\"");
    for name in ["crawlforge-core", "crawlforge-rules"] {
        let line = dependency_line(&manifest, name)
            .unwrap_or_else(|| panic!("{name} is missing from [workspace.dependencies]"));
        assert!(
            line.contains(&expected),
            "Cargo.toml: {name} must carry {expected} (the workspace version), found: {line}"
        );
    }
}

#[test]
fn package_json_follows_the_workspace_version() {
    let Some(package) = repo_file("package.json") else {
        eprintln!("not inside the CrawlForge repository: nothing to check");
        return;
    };
    let expected = format!("\"version\": \"{VERSION}\"");
    assert!(package.contains(&expected), "package.json must carry {expected}");
}

#[test]
fn the_dependency_line_is_found_by_name_and_not_by_prefix() {
    // `crawlforge-core` must not match a line for `crawlforge-core-extra`, or the check above
    // could read the wrong dependency and pass.
    let manifest = "crawlforge-core-extra = { path = \"x\" }\ncrawlforge-core = { path = \"y\", version = \"1.0.0\" }\n";
    assert_eq!(
        dependency_line(manifest, "crawlforge-core"),
        Some("crawlforge-core = { path = \"y\", version = \"1.0.0\" }")
    );
    assert_eq!(dependency_line(manifest, "crawlforge-rules"), None);
}
