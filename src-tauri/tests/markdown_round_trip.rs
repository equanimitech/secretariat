//! Integration smoke: file_io read → mutate → write round-trip on a real disk.

use std::path::PathBuf;

#[test]
fn round_trip_via_file_io() {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join("note.md");
    std::fs::write(&path, b"---\ntitle: T\n---\n# H\n").unwrap();

    let read = secretariat_lib::markdown::file_io::read_file(&path).unwrap();
    assert!(read.content.contains("title: T"));

    let new_sha = secretariat_lib::markdown::file_io::write_file(
        &path,
        "---\ntitle: T2\n---\n# H\n",
        &read.sha256,
    )
    .unwrap();
    assert_ne!(new_sha, read.sha256);

    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("title: T2"));
}
