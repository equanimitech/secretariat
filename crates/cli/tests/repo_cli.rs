//! `sec repo add → list → remove` round-trip under a temp SECRETARIAT_HOME.

use std::process::Command;

use tempfile::TempDir;

fn sec() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sec"))
}

#[test]
fn repo_add_list_remove_roundtrip() {
    let home = TempDir::new().unwrap();
    // The repo to enroll: a git dir inside the temp home.
    let repo = home.path().join("themia");
    std::fs::create_dir_all(repo.join(".git")).unwrap();

    // add
    let out = sec()
        .env("SECRETARIAT_HOME", home.path())
        .args(["repo", "add"])
        .arg(&repo)
        .args(["--role", "project", "--tag", "themia"])
        .output()
        .unwrap();
    assert!(out.status.success(), "add failed: {out:?}");

    // list --json contains the repo
    let out = sec()
        .env("SECRETARIAT_HOME", home.path())
        .args(["repo", "list", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("themia"), "list missing repo: {stdout}");

    // remove
    let out = sec()
        .env("SECRETARIAT_HOME", home.path())
        .args(["repo", "remove"])
        .arg(&repo)
        .output()
        .unwrap();
    assert!(out.status.success());

    // list is now empty
    let out = sec()
        .env("SECRETARIAT_HOME", home.path())
        .args(["repo", "list", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "post-remove list failed: {out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "[]");
}
