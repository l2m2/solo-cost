use rusqlite::Connection;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub conn: Mutex<Option<Connection>>,
}
