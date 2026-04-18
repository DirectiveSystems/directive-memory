use dm_core::{config::Config, Core};
use tempfile::tempdir;

#[tokio::test]
#[cfg(unix)]
async fn read_file_rejects_symlink_to_outside_root() {
    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    std::fs::write(outside.path().join("secret.md"), "TOP SECRET").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.md"),
        mem.join("secret.md")).unwrap();

    let mut cfg = Config::default();
    cfg.memory_dir = mem.clone();
    cfg.db_path = dir.path().join("db.sqlite");
    cfg.api_key = "k".into();
    let core = Core::open(cfg).await.unwrap();

    let res = core.read_file("secret.md");
    assert!(res.is_err(), "read through symlink must be rejected");
}
