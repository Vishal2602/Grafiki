pub mod schema;

use std::path::Path;

use rusqlite::Connection;

use crate::Result;

pub fn open_project_database(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    apply_pragmas(&connection)?;
    Ok(connection)
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
