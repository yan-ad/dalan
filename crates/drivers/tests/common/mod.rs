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
