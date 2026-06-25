pub mod schema;

use std::path::Path;

use rusqlite::Connection;

use crate::Result;

pub fn open_project_database(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    apply_pragmas(&connection)?;
    restrict_db_file_permissions(path);
    Ok(connection)
}

/// Best-effort: make the SQLite database and its WAL/SHM sidecars owner-only
/// (0600) on Unix. The database holds project memory that can include captured
/// code/content, so it should not be world-readable.
#[cfg(unix)]
fn restrict_db_file_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    for suffix in ["", "-wal", "-shm"] {
        let target = if suffix.is_empty() {
            path.to_path_buf()
        } else {
            let mut name = path.as_os_str().to_owned();
            name.push(suffix);
            std::path::PathBuf::from(name)
        };
        if target.exists() {
            let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600));
        }
    }
}

#[cfg(not(unix))]
fn restrict_db_file_permissions(_path: &Path) {}

/// Best-effort: make a directory owner-only (0700) on Unix. Used for the
/// Grafiki home directory so other local users cannot list project databases.
pub fn restrict_dir_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

pub fn apply_pragmas(connection: &Connection) -> Result<()> {
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -64000;
        PRAGMA temp_store = MEMORY;
        PRAGMA journal_mode = WAL;
        ",
    )?;

    Ok(())
}
