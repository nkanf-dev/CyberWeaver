#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use axum::{routing::get, Router};
use sea_orm::{Database, DatabaseBackend, DatabaseConnection, QueryResult, Statement, ConnectionTrait};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::sync::OnceCell;

static DB: OnceCell<DatabaseConnection> = OnceCell::const_new();

async fn init_db() -> Result<(), sea_orm::DbErr> {
    let db_url = "sqlite://cyberweaver.db?mode=rwc";
    // ✅ 修复：使用 :: 调用关联函数
    let db = Database::connect(db_url).await?;
    let create_table_sql = "CREATE TABLE IF NOT EXISTS nodes (id TEXT PRIMARY KEY, type TEXT NOT NULL, x REAL NOT NULL, y REAL NOT NULL, content TEXT NOT NULL);";
    db.execute(Statement::from_string(DatabaseBackend::Sqlite, create_table_sql.to_string())).await?;
    let _ = DB.set(db);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct NodePayload { pub id: String, pub r#type: String, pub x: f64, pub y: f64, pub content: String }

#[derive(Debug, Serialize)]
struct NodeModel { pub id: String, pub r#type: String, pub x: f64, pub y: f64, pub content: String }

impl NodeModel {
    fn from_row(row: QueryResult) -> Self {
        Self {
            id: row.try_get("", "id").unwrap_or_default(),
            r#type: row.try_get("", "type").unwrap_or_default(),
            x: row.try_get("", "x").unwrap_or(0.0),
            y: row.try_get("", "y").unwrap_or(0.0),
            content: row.try_get("", "content").unwrap_or_default(),
        }
    }
}

#[tauri::command]
async fn save_node(node: NodePayload) -> Result<(), String> {
    let db = DB.get().ok_or("Database Not Ready")?;
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT OR REPLACE INTO nodes (id, type, x, y, content) VALUES ($1, $2, $3, $4, $5)",
        vec![node.id.into(), node.r#type.into(), node.x.into(), node.y.into(), node.content.into()],
    )).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_nodes() -> Result<Vec<NodeModel>, String> {
    let db = DB.get().ok_or("Database Not Ready")?;
    let rows = db.query_all(Statement::from_string(DatabaseBackend::Sqlite, "SELECT * FROM nodes".to_string())).await.map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(NodeModel::from_row).collect())
}

#[tokio::main]
async fn main() {
    tokio::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let app = Router::new().route("/ping", get(|| async { "pong" }));
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });
    let _ = init_db().await;
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![save_node, get_nodes])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}