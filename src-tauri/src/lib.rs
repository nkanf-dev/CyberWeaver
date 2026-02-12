use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DbErr, QueryResult, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};
use tauri::{AppHandle, Manager, State};

const DB_FILE_NAME: &str = "cyberweaver.db";

#[derive(Clone)]
struct AppState {
    db: DatabaseConnection,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NodePayload {
    id: String,
    #[serde(rename = "type")]
    node_type: String,
    x: f64,
    y: f64,
    content: String,
    width: Option<f64>,
    height: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct NodeModel {
    id: String,
    #[serde(rename = "type")]
    node_type: String,
    x: f64,
    y: f64,
    content: String,
    width: Option<f64>,
    height: Option<f64>,
}

impl NodeModel {
    fn from_row(row: QueryResult) -> Self {
        Self {
            id: row.try_get("", "id").unwrap_or_default(),
            node_type: row.try_get("", "type").unwrap_or_default(),
            x: row.try_get("", "x").unwrap_or(0.0),
            y: row.try_get("", "y").unwrap_or(0.0),
            content: row.try_get("", "content").unwrap_or_default(),
            width: row.try_get("", "width").ok(),
            height: row.try_get("", "height").ok(),
        }
    }
}

fn normalize_shape_id(raw: &str) -> String {
    let trimmed = raw.trim();

    if trimmed.starts_with("shape:") {
        trimmed.to_owned()
    } else {
        format!("shape:{trimmed}")
    }
}

fn normalize_node_type(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "geo" => Some("geo"),
        "text" => Some("text"),
        "note" => Some("note"),
        _ => None,
    }
}

fn validate_node_payload(node: &NodePayload) -> Result<(), String> {
    if node.id.trim().is_empty() {
        return Err("node.id must not be empty".to_owned());
    }

    if normalize_node_type(&node.node_type).is_none() {
        return Err(format!("unsupported node type: {}", node.node_type));
    }

    if !node.x.is_finite() || !node.y.is_finite() {
        return Err("node coordinates must be finite numbers".to_owned());
    }

    if node
        .width
        .is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        return Err("node.width must be a positive finite number when provided".to_owned());
    }

    if node
        .height
        .is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        return Err("node.height must be a positive finite number when provided".to_owned());
    }

    Ok(())
}

fn sqlite_url_from_path(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    format!("sqlite://{raw}?mode=rwc")
}

async fn connect_database_from_path(path: &Path) -> Result<DatabaseConnection, DbErr> {
    Database::connect(sqlite_url_from_path(path)).await
}

async fn connect_database(app_handle: &AppHandle) -> Result<DatabaseConnection, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?;

    std::fs::create_dir_all(&app_data_dir).map_err(|err| err.to_string())?;

    connect_database_from_path(&app_data_dir.join(DB_FILE_NAME))
        .await
        .map_err(|err| err.to_string())
}

async fn ensure_column(
    db: &DatabaseConnection,
    column_name: &str,
    column_definition: &str,
) -> Result<(), DbErr> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA table_info(nodes);".to_owned(),
        ))
        .await?;

    let has_column = rows.into_iter().any(|row| {
        row.try_get::<String>("", "name")
            .map(|name| name == column_name)
            .unwrap_or(false)
    });

    if has_column {
        return Ok(());
    }

    let sql = format!("ALTER TABLE nodes ADD COLUMN {column_name} {column_definition};");
    db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await?;

    Ok(())
}

async fn init_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL,
            x REAL NOT NULL,
            y REAL NOT NULL,
            content TEXT NOT NULL,
            width REAL,
            height REAL,
            updated_at INTEGER NOT NULL DEFAULT 0
        );"
        .to_owned(),
    ))
    .await?;

    ensure_column(db, "width", "REAL").await?;
    ensure_column(db, "height", "REAL").await?;
    ensure_column(db, "updated_at", "INTEGER NOT NULL DEFAULT 0").await?;

    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "CREATE INDEX IF NOT EXISTS idx_nodes_type_updated_at ON nodes(type, updated_at);"
            .to_owned(),
    ))
    .await?;

    Ok(())
}

async fn list_nodes_internal(db: &DatabaseConnection) -> Result<Vec<NodeModel>, String> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT id, type, x, y, content, width, height
             FROM nodes
             WHERE type IN ('geo', 'text', 'note')
             ORDER BY updated_at ASC, id ASC;"
                .to_owned(),
        ))
        .await
        .map_err(|err| err.to_string())?;

    Ok(rows.into_iter().map(NodeModel::from_row).collect())
}

async fn upsert_nodes_internal(
    db: &DatabaseConnection,
    nodes: Vec<NodePayload>,
) -> Result<(), String> {
    if nodes.is_empty() {
        return Ok(());
    }

    for node in &nodes {
        validate_node_payload(node)?;
    }

    let txn = db.begin().await.map_err(|err| err.to_string())?;

    for node in nodes {
        let normalized_type = normalize_node_type(&node.node_type)
            .ok_or_else(|| format!("unsupported node type: {}", node.node_type))?;

        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO nodes (id, type, x, y, content, width, height, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, unixepoch())
             ON CONFLICT(id) DO UPDATE SET
               type = excluded.type,
               x = excluded.x,
               y = excluded.y,
               content = excluded.content,
               width = excluded.width,
               height = excluded.height,
               updated_at = unixepoch();"
                .to_owned(),
            vec![
                normalize_shape_id(&node.id).into(),
                normalized_type.into(),
                node.x.into(),
                node.y.into(),
                node.content.into(),
                node.width.into(),
                node.height.into(),
            ],
        ))
        .await
        .map_err(|err| err.to_string())?;
    }

    txn.commit().await.map_err(|err| err.to_string())?;

    Ok(())
}

fn normalize_delete_ids(ids: Vec<String>) -> Vec<String> {
    let mut deduped = BTreeSet::new();

    for id in ids {
        let trimmed = id.trim();

        if !trimmed.is_empty() {
            deduped.insert(normalize_shape_id(trimmed));
        }
    }

    deduped.into_iter().collect()
}

async fn delete_nodes_internal(db: &DatabaseConnection, ids: Vec<String>) -> Result<(), String> {
    let normalized_ids = normalize_delete_ids(ids);

    if normalized_ids.is_empty() {
        return Ok(());
    }

    let placeholders = vec!["?"; normalized_ids.len()].join(", ");
    let sql = format!("DELETE FROM nodes WHERE id IN ({placeholders});");

    let values = normalized_ids
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>();

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql,
        values,
    ))
    .await
    .map_err(|err| err.to_string())?;

    Ok(())
}

#[tauri::command]
async fn get_nodes(state: State<'_, AppState>) -> Result<Vec<NodeModel>, String> {
    list_nodes_internal(&state.db).await
}

#[tauri::command]
async fn upsert_nodes(state: State<'_, AppState>, nodes: Vec<NodePayload>) -> Result<(), String> {
    upsert_nodes_internal(&state.db, nodes).await
}

#[tauri::command]
async fn delete_nodes(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    delete_nodes_internal(&state.db, ids).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db = tauri::async_runtime::block_on(async {
                let db = connect_database(app.handle()).await?;
                init_schema(&db).await.map_err(|err| err.to_string())?;
                Ok::<DatabaseConnection, String>(db)
            })?;

            app.manage(AppState { db });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_nodes,
            upsert_nodes,
            delete_nodes
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("failed to connect sqlite");

        init_schema(&db).await.expect("failed to init schema");
        db
    }

    #[tokio::test]
    async fn upsert_and_get_nodes_roundtrip() {
        let db = create_test_db().await;

        upsert_nodes_internal(
            &db,
            vec![NodePayload {
                id: "artifact-1".to_owned(),
                node_type: "text".to_owned(),
                x: 12.0,
                y: 34.0,
                content: "IOC discovered".to_owned(),
                width: Some(200.0),
                height: None,
            }],
        )
        .await
        .expect("upsert should succeed");

        let rows = list_nodes_internal(&db)
            .await
            .expect("query should succeed");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "shape:artifact-1");
        assert_eq!(rows[0].node_type, "text");
        assert_eq!(rows[0].content, "IOC discovered");
        assert_eq!(rows[0].width, Some(200.0));
    }

    #[tokio::test]
    async fn delete_nodes_removes_rows() {
        let db = create_test_db().await;

        upsert_nodes_internal(
            &db,
            vec![NodePayload {
                id: "shape:artifact-2".to_owned(),
                node_type: "note".to_owned(),
                x: 0.0,
                y: 0.0,
                content: "temporary".to_owned(),
                width: None,
                height: None,
            }],
        )
        .await
        .expect("upsert should succeed");

        delete_nodes_internal(&db, vec!["artifact-2".to_owned()])
            .await
            .expect("delete should succeed");

        let rows = list_nodes_internal(&db)
            .await
            .expect("query should succeed");
        assert!(rows.is_empty());
    }

    #[test]
    fn validate_payload_rejects_unknown_shape_types() {
        let payload = NodePayload {
            id: "shape:x".to_owned(),
            node_type: "draw".to_owned(),
            x: 0.0,
            y: 0.0,
            content: String::new(),
            width: None,
            height: None,
        };

        let result = validate_node_payload(&payload);
        assert!(result.is_err());
    }
}
