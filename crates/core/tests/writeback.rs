use dm_core::writeback;
use std::fs;
use tempfile::tempdir;

#[test]
fn write_creates_file() {
    let dir = tempdir().unwrap();
    writeback::write_file(dir.path(), "notes.md", "# hi\nbody", false).unwrap();
    let body = fs::read_to_string(dir.path().join("notes.md")).unwrap();
    assert_eq!(body, "# hi\nbody");
}

#[test]
fn append_preserves_trailing_newline() {
    let dir = tempdir().unwrap();
    writeback::write_file(dir.path(), "x.md", "line one", false).unwrap();
    writeback::write_file(dir.path(), "x.md", "line two", true).unwrap();
    let body = fs::read_to_string(dir.path().join("x.md")).unwrap();
    assert!(body.starts_with("line one\n"));
    assert!(body.contains("line two"));
}

#[test]
fn rejects_path_traversal() {
    let dir = tempdir().unwrap();
    assert!(writeback::write_file(dir.path(), "../evil.md", "x", false).is_err());
    assert!(writeback::write_file(dir.path(), "a/../b.md", "x", false).is_err());
    assert!(writeback::write_file(dir.path(), "/etc/passwd.md", "x", false).is_err());
}

#[test]
fn rejects_non_markdown_extension() {
    let dir = tempdir().unwrap();
    assert!(writeback::write_file(dir.path(), "note.txt", "x", false).is_err());
}

#[test]
fn add_fact_creates_file_and_section_when_missing() {
    let dir = tempdir().unwrap();
    writeback::add_fact(dir.path(), "learnings.md", "## Patterns", "use sqlx").unwrap();
    let body = fs::read_to_string(dir.path().join("learnings.md")).unwrap();
    assert!(body.contains("# Learnings"));
    assert!(body.contains("## Patterns"));
    assert!(body.contains("- use sqlx"));
}

#[test]
fn add_fact_appends_under_existing_section() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("x.md"),
        "# X\n\n## Patterns\n- old one\n\n## Other\ndata\n").unwrap();
    writeback::add_fact(dir.path(), "x.md", "## Patterns", "new fact").unwrap();
    let body = fs::read_to_string(dir.path().join("x.md")).unwrap();
    let patterns_idx = body.find("## Patterns").unwrap();
    let other_idx    = body.find("## Other").unwrap();
    let slice = &body[patterns_idx..other_idx];
    assert!(slice.contains("- old one"));
    assert!(slice.contains("- new fact"));
}

#[test]
fn add_fact_appends_new_section_when_heading_absent() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("x.md"), "# X\n\nbody\n").unwrap();
    writeback::add_fact(dir.path(), "x.md", "## New", "added").unwrap();
    let body = fs::read_to_string(dir.path().join("x.md")).unwrap();
    assert!(body.contains("## New"));
    assert!(body.contains("- added"));
}

#[test]
#[cfg(unix)]
fn rejects_write_through_symlink_to_outside_root() {
    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    // Create a symlink INSIDE memory dir pointing to a file outside it.
    std::os::unix::fs::symlink(outside.path().join("target.md"),
        dir.path().join("sneak.md")).unwrap();
    let err = writeback::write_file(dir.path(), "sneak.md", "x", false);
    assert!(err.is_err(), "must reject writing through a symlink");
    assert!(!outside.path().join("target.md").exists(),
        "target file must not have been created");
}

#[test]
#[cfg(unix)]
fn rejects_write_into_directory_symlink() {
    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    // Symlink a subdirectory inside memory_dir pointing at an outside directory.
    std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).unwrap();
    let err = writeback::write_file(dir.path(), "escape/sneaky.md", "x", false);
    assert!(err.is_err());
    assert!(!outside.path().join("sneaky.md").exists(),
        "file must not have been written into the symlinked directory");
}
