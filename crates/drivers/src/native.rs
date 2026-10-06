//! Engine-routed operations. Native JSON consoles are not SQL executors.
use crate::{
    BrowseRequest, CatalogSnapshot, ColumnInfo, ConnectionReport, DbEngine, SourceProfile,
    TableInfo, TablePage,
};
use anyhow::Result;
macro_rules! route {($profile:expr,$function:ident $(,$argument:expr)*) => {{crate::versions::selected($profile.engine)?;match $profile.engine {
DbEngine::MySql|DbEngine::MariaDb=>crate::mysql::$function($profile $(,$argument)*).await,
DbEngine::PostgreSql=>crate::postgres::$function($profile $(,$argument)*).await,
DbEngine::MongoDb=>crate::mongo::$function($profile $(,$argument)*).await,
DbEngine::Redis=>crate::redis_driver::$function($profile $(,$argument)*).await,
}}};}
pub async fn test_connection(profile: &SourceProfile, password: &str) -> Result<ConnectionReport> {
    route!(profile, test_connection, password)
}
pub async fn discover_catalog(profile: &SourceProfile, password: &str) -> Result<CatalogSnapshot> {
    route!(profile, discover_catalog, password)
}
pub async fn tables(
    profile: &SourceProfile,
    password: &str,
    database: &str,
) -> Result<Vec<TableInfo>> {
    route!(profile, tables, password, database)
}
pub async fn columns(
    profile: &SourceProfile,
    password: &str,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>> {
    route!(profile, columns, password, database, table)
}
pub async fn browse(
    profile: &SourceProfile,
    password: &str,
    request: &BrowseRequest,
) -> Result<TablePage> {
    route!(profile, browse, password, request)
}

pub fn is_browsable_kind(kind: &str) -> bool {
    matches!(
        kind,
        "BASE TABLE" | "COLLECTION" | "STRING" | "HASH" | "LIST" | "SET" | "ZSET"
    )
}
