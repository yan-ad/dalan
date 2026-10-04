use dalan_drivers::{BrowseRequest, SortDirection, SourceProfile, TableSort, browse};

/// Shared, read-only fixture checks for every engine and successful transport.
/// Fixture names are Alice, Bob, and 100%_literal! (IDs 1, 2, and 3).
pub async fn sorted_pages(
    profile: &SourceProfile,
    password: &str,
    base: &BrowseRequest,
) -> anyhow::Result<()> {
    for (column, direction, expected) in [
        ("name", SortDirection::Ascending, ["3", "1", "2"]),
        ("name", SortDirection::Descending, ["2", "1", "3"]),
        ("id", SortDirection::Ascending, ["1", "2", "3"]),
        ("id", SortDirection::Descending, ["3", "2", "1"]),
    ] {
        let mut request = BrowseRequest {
            sort: Some(TableSort {
                column: column.into(),
                direction,
            }),
            filter: None,
            limit: 1,
            offset: 0,
            ..base.clone()
        };
        for (index, id) in expected.iter().enumerate() {
            let page = browse(profile, password, &request).await?;
            assert_eq!(page.offset, index as u64);
            assert_eq!(page.rows.len(), 1);
            let id_index = page.columns.iter().position(|c| c.name == "id").unwrap();
            assert_eq!(
                page.rows[0][id_index].display(),
                *id,
                "{column} {direction:?}"
            );
            assert_eq!(page.has_more, index < 2);
            assert_eq!(page.next_offset, (index < 2).then_some(index as u64 + 1));
            if let Some(offset) = page.next_offset {
                request.offset = offset;
            }
        }
    }
    for column in ["unknown", "name DESC; DROP TABLE contact --", "LOWER(name)"] {
        let request = BrowseRequest {
            sort: Some(TableSort {
                column: column.into(),
                direction: SortDirection::Ascending,
            }),
            ..base.clone()
        };
        let error = browse(profile, password, &request).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Sort column is not in table metadata")
        );
    }
    Ok(())
}

/// Repeated metadata-only snapshots, including scoped discovery and failure recovery.
pub async fn full_catalog(
    profile: &SourceProfile,
    password: &str,
    database: &str,
) -> anyhow::Result<()> {
    use dalan_drivers::{discover_catalog, test_connection};
    let mut all = profile.clone();
    all.database = None;
    let visible = test_connection(&all, password).await?.databases;
    for _ in 0..2 {
        let snapshot = discover_catalog(&all, password).await?;
        assert_eq!(
            snapshot
                .databases
                .iter()
                .map(|db| db.name.clone())
                .collect::<Vec<_>>(),
            visible
        );
        // The fixture reader has no grants on application schemas other than its fixture.
        assert!(snapshot.databases.iter().all(|db| db.name == database
            || matches!(
                db.name.as_str(),
                "information_schema" | "performance_schema" | "sys" | "mysql"
            )));
        let fixture = snapshot
            .databases
            .iter()
            .find(|db| db.name == database)
            .unwrap();
        assert_fixture(fixture);
        let serialized = serde_json::to_string(&snapshot)?;
        for raw_value in [
            "alice@example.com",
            "12.345678901234567890",
            "18446744073709551615",
            "0x414200ff",
        ] {
            assert!(!serialized.contains(raw_value));
        }
        assert!(!serialized.contains("\"rows\""));
    }
    let mut scoped = all.clone();
    scoped.database = Some(database.into());
    for _ in 0..2 {
        let snapshot = discover_catalog(&scoped, password).await?;
        assert_eq!(snapshot.databases.len(), 1);
        assert_eq!(snapshot.databases[0].name, database);
        assert_fixture(&snapshot.databases[0]);
    }
    scoped.database = Some("dalan_nonexistent_catalog_fixture".into());
    let error = discover_catalog(&scoped, password).await.unwrap_err();
    assert!(!format!("{error:#}").contains(password));
    // A failed snapshot must not poison future sessions or return partial success.
    scoped.database = Some(database.into());
    assert_fixture(&discover_catalog(&scoped, password).await?.databases[0]);
    Ok(())
}
fn assert_fixture(database: &dalan_drivers::DatabaseCatalog) {
    assert!(
        database
            .tables
            .iter()
            .any(|t| t.name == "contact" && t.kind == "BASE TABLE")
    );
    assert!(
        database
            .tables
            .iter()
            .any(|t| t.name == "contact_view" && t.kind == "VIEW")
    );
    assert!(
        database
            .tables
            .windows(2)
            .all(|pair| pair[0].name <= pair[1].name)
    );
}
