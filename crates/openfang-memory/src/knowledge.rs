//! Knowledge graph backed by SQLite.
//!
//! Stores entities and relations with support for graph pattern queries.

use chrono::Utc;
use openfang_types::error::{OpenFangError, OpenFangResult};
use openfang_types::memory::{
    Entity, EntityType, GraphMatch, GraphPattern, Relation, RelationType,
};
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Knowledge graph store backed by SQLite.
#[derive(Clone)]
pub struct KnowledgeStore {
    conn: Arc<Mutex<Connection>>,
}

impl KnowledgeStore {
    /// Create a new knowledge store wrapping the given connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Add an entity to the knowledge graph.
    pub fn add_entity(&self, entity: Entity) -> OpenFangResult<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Internal(e.to_string()))?;
        // Choose the row id. Callers that pass an explicit id upsert on it.
        // Callers that leave the id empty (the `knowledge_add_entity` tool
        // always does) are de-duplicated by NAME: if an entity with the same
        // name already exists we reuse its id and UPDATE it, instead of
        // inserting a brand-new UUID row every time. Without this the graph
        // accumulated a fresh row for the same "NVIDIA"/"Boston Dynamics" on
        // every collection sweep, because `ON CONFLICT(id)` can never fire on
        // a freshly-generated UUID.
        let id = if entity.id.is_empty() {
            let existing: Option<String> = conn
                .query_row(
                    "SELECT id FROM entities WHERE name = ?1 LIMIT 1",
                    rusqlite::params![entity.name],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| OpenFangError::Memory(e.to_string()))?;
            existing.unwrap_or_else(|| Uuid::new_v4().to_string())
        } else {
            entity.id.clone()
        };
        // Store a flat, queryable type string ("organization", "product") —
        // not serde_json of the enum, which wrote known variants as quoted
        // scalars and custom ones as objects, so the column could not be
        // cleanly grouped/filtered or imported into a graph DB.
        let entity_type_str = entity.entity_type.to_string();
        let props_str = serde_json::to_string(&entity.properties)
            .map_err(|e| OpenFangError::Serialization(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        // On conflict, keep the original created_at (first-seen time) and only
        // refresh the mutable columns + updated_at.
        conn.execute(
            "INSERT INTO entities (id, entity_type, name, properties, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 entity_type = ?2, name = ?3, properties = ?4, updated_at = ?5",
            rusqlite::params![id, entity_type_str, entity.name, props_str, now],
        )
        .map_err(|e| OpenFangError::Memory(e.to_string()))?;
        Ok(id)
    }

    /// Add a relation between two entities.
    pub fn add_relation(&self, relation: Relation) -> OpenFangResult<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Internal(e.to_string()))?;
        let id = Uuid::new_v4().to_string();
        let rel_type_str = relation.relation.to_string();
        let props_str = serde_json::to_string(&relation.properties)
            .map_err(|e| OpenFangError::Serialization(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO relations (id, source_entity, relation_type, target_entity, properties, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                relation.source,
                rel_type_str,
                relation.target,
                props_str,
                relation.confidence as f64,
                now,
            ],
        )
        .map_err(|e| OpenFangError::Memory(e.to_string()))?;
        Ok(id)
    }

    /// Query the knowledge graph with a pattern.
    pub fn query_graph(&self, pattern: GraphPattern) -> OpenFangResult<Vec<GraphMatch>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Internal(e.to_string()))?;

        let mut sql = String::from(
            "SELECT
                s.id, s.entity_type, s.name, s.properties, s.created_at, s.updated_at,
                r.id, r.source_entity, r.relation_type, r.target_entity, r.properties, r.confidence, r.created_at,
                t.id, t.entity_type, t.name, t.properties, t.created_at, t.updated_at
             FROM relations r
             JOIN entities s ON (r.source_entity = s.id OR r.source_entity = s.name)
             JOIN entities t ON (r.target_entity = t.id OR r.target_entity = t.name)
             WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref source) = pattern.source {
            sql.push_str(&format!(" AND (s.id = ?{} OR s.name = ?{})", idx, idx + 1));
            params.push(Box::new(source.clone()));
            params.push(Box::new(source.clone()));
            idx += 2;
        }
        if let Some(ref relation) = pattern.relation {
            // Must match the storage format written by add_relation (flat
            // string), or relation filtering silently returns nothing.
            let rel_str = relation.to_string();
            sql.push_str(&format!(" AND r.relation_type = ?{idx}"));
            params.push(Box::new(rel_str));
            idx += 1;
        }
        if let Some(ref target) = pattern.target {
            sql.push_str(&format!(" AND (t.id = ?{} OR t.name = ?{})", idx, idx + 1));
            params.push(Box::new(target.clone()));
            params.push(Box::new(target.clone()));
            idx += 2;
        }
        let _ = idx;

        sql.push_str(" LIMIT 100");

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(RawGraphRow {
                    s_id: row.get(0)?,
                    s_type: row.get(1)?,
                    s_name: row.get(2)?,
                    s_props: row.get(3)?,
                    s_created: row.get(4)?,
                    s_updated: row.get(5)?,
                    r_id: row.get(6)?,
                    r_source: row.get(7)?,
                    r_type: row.get(8)?,
                    r_target: row.get(9)?,
                    r_props: row.get(10)?,
                    r_confidence: row.get(11)?,
                    r_created: row.get(12)?,
                    t_id: row.get(13)?,
                    t_type: row.get(14)?,
                    t_name: row.get(15)?,
                    t_props: row.get(16)?,
                    t_created: row.get(17)?,
                    t_updated: row.get(18)?,
                })
            })
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;

        let mut matches = Vec::new();
        for row_result in rows {
            let r = row_result.map_err(|e| OpenFangError::Memory(e.to_string()))?;
            matches.push(GraphMatch {
                source: parse_entity(
                    &r.s_id,
                    &r.s_type,
                    &r.s_name,
                    &r.s_props,
                    &r.s_created,
                    &r.s_updated,
                ),
                relation: parse_relation(
                    &r.r_source,
                    &r.r_type,
                    &r.r_target,
                    &r.r_props,
                    r.r_confidence,
                    &r.r_created,
                ),
                target: parse_entity(
                    &r.t_id,
                    &r.t_type,
                    &r.t_name,
                    &r.t_props,
                    &r.t_created,
                    &r.t_updated,
                ),
            });
        }
        Ok(matches)
    }
}

/// Raw row from a graph query.
struct RawGraphRow {
    s_id: String,
    s_type: String,
    s_name: String,
    s_props: String,
    s_created: String,
    s_updated: String,
    r_id: String,
    r_source: String,
    r_type: String,
    r_target: String,
    r_props: String,
    r_confidence: f64,
    r_created: String,
    t_id: String,
    t_type: String,
    t_name: String,
    t_props: String,
    t_created: String,
    t_updated: String,
}

// Suppress the unused field warning — r_id is part of the schema
impl RawGraphRow {
    #[allow(dead_code)]
    fn relation_id(&self) -> &str {
        &self.r_id
    }
}

fn parse_entity(
    id: &str,
    etype: &str,
    name: &str,
    props: &str,
    created: &str,
    updated: &str,
) -> Entity {
    let entity_type = EntityType::from_db_str(etype);
    let properties: HashMap<String, serde_json::Value> =
        serde_json::from_str(props).unwrap_or_default();
    let created_at = chrono::DateTime::parse_from_rfc3339(created)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let updated_at = chrono::DateTime::parse_from_rfc3339(updated)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    Entity {
        id: id.to_string(),
        entity_type,
        name: name.to_string(),
        properties,
        created_at,
        updated_at,
    }
}

fn parse_relation(
    source: &str,
    rtype: &str,
    target: &str,
    props: &str,
    confidence: f64,
    created: &str,
) -> Relation {
    let relation = RelationType::from_db_str(rtype);
    let properties: HashMap<String, serde_json::Value> =
        serde_json::from_str(props).unwrap_or_default();
    let created_at = chrono::DateTime::parse_from_rfc3339(created)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    Relation {
        source: source.to_string(),
        relation,
        target: target.to_string(),
        properties,
        confidence: confidence as f32,
        created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::run_migrations;

    fn setup() -> KnowledgeStore {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        KnowledgeStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn test_add_and_query_entity() {
        let store = setup();
        let id = store
            .add_entity(Entity {
                id: String::new(),
                entity_type: EntityType::Person,
                name: "Alice".to_string(),
                properties: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        assert!(!id.is_empty());
    }

    fn count_entities(store: &KnowledgeStore) -> i64 {
        let conn = store.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn test_add_entity_dedupes_by_name() {
        let store = setup();
        // Same name added three times the way the tool does it — empty id.
        let mut ids = Vec::new();
        for _ in 0..3 {
            ids.push(
                store
                    .add_entity(Entity {
                        id: String::new(),
                        entity_type: EntityType::Organization,
                        name: "NVIDIA".to_string(),
                        properties: HashMap::new(),
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    })
                    .unwrap(),
            );
        }
        // All three calls resolve to the SAME id and only ONE row exists.
        assert_eq!(ids[0], ids[1]);
        assert_eq!(ids[1], ids[2]);
        assert_eq!(count_entities(&store), 1, "expected one deduped NVIDIA row");

        // A different name is still a separate row.
        store
            .add_entity(Entity {
                id: String::new(),
                entity_type: EntityType::Organization,
                name: "Boston Dynamics".to_string(),
                properties: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        assert_eq!(count_entities(&store), 2);
    }

    fn raw_entity_type(store: &KnowledgeStore, name: &str) -> String {
        let conn = store.conn.lock().unwrap();
        conn.query_row(
            "SELECT entity_type FROM entities WHERE name = ?1",
            rusqlite::params![name],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn test_entity_type_stored_flat() {
        let store = setup();
        store
            .add_entity(Entity {
                id: String::new(),
                entity_type: EntityType::Organization,
                name: "NVIDIA".to_string(),
                properties: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        store
            .add_entity(Entity {
                id: String::new(),
                entity_type: EntityType::Custom("product".to_string()),
                name: "Isaac GR00T".to_string(),
                properties: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        // Flat strings — no quotes, no {"custom":...} objects.
        assert_eq!(raw_entity_type(&store, "NVIDIA"), "organization");
        assert_eq!(raw_entity_type(&store, "Isaac GR00T"), "product");
    }

    #[test]
    fn test_entity_type_from_db_str_reads_all_forms() {
        // Flat (new), quoted scalar (legacy), and object (legacy custom).
        assert_eq!(
            EntityType::from_db_str("organization"),
            EntityType::Organization
        );
        assert_eq!(
            EntityType::from_db_str("\"organization\""),
            EntityType::Organization
        );
        assert_eq!(
            EntityType::from_db_str("product"),
            EntityType::Custom("product".to_string())
        );
        assert_eq!(
            EntityType::from_db_str("{\"custom\":\"product\"}"),
            EntityType::Custom("product".to_string())
        );
    }

    #[test]
    fn test_relation_stored_by_name_is_queryable() {
        // The knowledge_add_relation tool passes whatever source/target strings
        // the model supplied — i.e. entity NAMES — while add_entity keys rows by
        // a generated UUID. Joining only on `id` therefore matched nothing and
        // silently hid the entire graph (a live DB had 348 relations of which
        // exactly 1 was reachable). The join must accept id OR name.
        let store = setup();
        store
            .add_entity(Entity {
                id: String::new(),
                entity_type: EntityType::Organization,
                name: "NVIDIA".to_string(),
                properties: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        store
            .add_entity(Entity {
                id: String::new(),
                entity_type: EntityType::Custom("product".to_string()),
                name: "Isaac GR00T".to_string(),
                properties: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        // Relation references entities BY NAME, exactly as the tool does.
        store
            .add_relation(Relation {
                source: "NVIDIA".to_string(),
                relation: RelationType::Custom("develops".to_string()),
                target: "Isaac GR00T".to_string(),
                properties: HashMap::new(),
                confidence: 1.0,
                created_at: Utc::now(),
            })
            .unwrap();

        let matches = store
            .query_graph(GraphPattern {
                source: Some("NVIDIA".to_string()),
                relation: None,
                target: None,
                max_depth: 1,
            })
            .unwrap();
        assert_eq!(matches.len(), 1, "name-keyed relation must be queryable");
        assert_eq!(matches[0].source.name, "NVIDIA");
        assert_eq!(matches[0].target.name, "Isaac GR00T");

        // And an unfiltered query must see it too.
        let all = store
            .query_graph(GraphPattern {
                source: None,
                relation: None,
                target: None,
                max_depth: 1,
            })
            .unwrap();
        assert_eq!(all.len(), 1, "unfiltered query must see the edge");
    }

    #[test]
    fn test_add_relation_and_query() {
        let store = setup();
        let alice_id = store
            .add_entity(Entity {
                id: "alice".to_string(),
                entity_type: EntityType::Person,
                name: "Alice".to_string(),
                properties: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        let company_id = store
            .add_entity(Entity {
                id: "acme".to_string(),
                entity_type: EntityType::Organization,
                name: "Acme Corp".to_string(),
                properties: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        store
            .add_relation(Relation {
                source: alice_id.clone(),
                relation: RelationType::WorksAt,
                target: company_id,
                properties: HashMap::new(),
                confidence: 0.95,
                created_at: Utc::now(),
            })
            .unwrap();

        let matches = store
            .query_graph(GraphPattern {
                source: Some(alice_id),
                relation: Some(RelationType::WorksAt),
                target: None,
                max_depth: 1,
            })
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].target.name, "Acme Corp");
    }
}
