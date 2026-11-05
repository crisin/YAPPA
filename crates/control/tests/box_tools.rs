//! The commands an operator runs on the box: backup and health, against a real file and a
//! real socket.

use control::cli::{check_health, start};
use control::{open_db, store, Config};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

/// A folder of its own per test, removed at the end. The random part keeps a reused pid from
/// finding an old database.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let mut suffix = [0u8; 4];
        getrandom::fill(&mut suffix).unwrap();
        let path = std::env::temp_dir().join(format!(
            "yappa-box-{}-{}",
            std::process::id(),
            hex::encode(suffix)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn open(path: &Path) -> SqlitePool {
    open_db(&format!("sqlite://{}", path.display()))
        .await
        .unwrap()
}

async fn channel_names(db: &SqlitePool) -> Vec<String> {
    store::channels(db)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect()
}

#[tokio::test]
async fn a_backup_holds_what_is_still_in_the_wal() {
    let dir = TempDir::new();
    let db = open(&dir.file("yappa.db")).await;
    store::create_channel(&db, "raid", "Raid", "main")
        .await
        .unwrap();

    // The pool stays open, so the write has not been checkpointed into yappa.db.
    let target = dir.file("backup.db");
    store::backup_to(&db, &target).await.unwrap();

    let restored = open(&target).await;
    assert!(channel_names(&restored).await.contains(&"Raid".into()));
    assert!(!dir.file("backup.db.tmp").exists());
}

#[tokio::test]
async fn a_second_backup_replaces_the_first() {
    let dir = TempDir::new();
    let db = open(&dir.file("yappa.db")).await;
    let target = dir.file("nightly.db");
    store::backup_to(&db, &target).await.unwrap();

    store::create_channel(&db, "raid", "Raid", "main")
        .await
        .unwrap();
    store::backup_to(&db, &target).await.unwrap();

    let restored = open(&target).await;
    assert!(channel_names(&restored).await.contains(&"Raid".into()));
}

#[tokio::test]
async fn a_leftover_from_a_failed_run_does_not_block_the_next_backup() {
    let dir = TempDir::new();
    let db = open(&dir.file("yappa.db")).await;
    std::fs::write(dir.file("nightly.db.tmp"), b"half a file").unwrap();

    store::backup_to(&db, &dir.file("nightly.db"))
        .await
        .unwrap();

    assert!(!dir.file("nightly.db.tmp").exists());
    assert!(dir.file("nightly.db").exists());
}

#[tokio::test]
async fn a_failed_backup_leaves_the_old_one_alone() {
    let dir = TempDir::new();
    let db = open(&dir.file("yappa.db")).await;
    let missing = dir.file("no-such-folder").join("nightly.db");

    assert!(store::backup_to(&db, &missing).await.is_err());
    assert!(!missing.exists());
}

#[tokio::test]
async fn health_says_ok_for_a_running_server_and_fails_without_one() {
    let (addr, _server) = start(Config::local("sqlite::memory:")).await.unwrap();
    check_health(addr).await.unwrap();

    // Nothing listens on a port that was just free.
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = free.local_addr().unwrap();
    drop(free);
    assert!(check_health(dead).await.is_err());
}
