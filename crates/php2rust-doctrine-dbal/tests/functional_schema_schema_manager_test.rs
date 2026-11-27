//! Port of tests/Functional/Schema/SchemaManagerTest.php on `pdo_sqlite`
//! in memory. Names and assertions are PHPUnit's; a data provider's (or
//! `TestWith`'s) cases run in one test, in order, without the cases that
//! upstream marks incomplete or skipped on `SQLite`. The tests that
//! upstream skips on `SQLite` altogether are not here (parity marks them
//! n/a). The metadata provider is the `SQLite` platform's (PHP takes it
//! from the connection's platform, which is the same class).
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/SchemaManagerTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::deprecations;
use php2rust_doctrine_dbal::platforms::{AbstractPlatform, SQLitePlatform};
use php2rust_doctrine_dbal::schema::metadata::MetadataProvider;
use php2rust_doctrine_dbal::schema::name::{OptionallyQualifiedName, UnqualifiedName};
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, Column, ForeignKeyConstraint, Index, IndexType, Sequence, Table,
    UniqueConstraint,
};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, Exception};
use schema_manager_support::{
    assert_index_equals, column, drop_and_create_table, drop_table_if_exists, exec, foreign_key,
    index, platform, primary_key, schema_manager, sqlite, to_quoted_unqualified_name,
};

const LINK_6768: &str = "https://github.com/doctrine/dbal/pull/6768";
const NOT_SUPPORTED: &str = "Doctrine\\DBAL\\Platforms\\Exception\\NotSupported";
const UNSUPPORTED_NAME: &str = "Doctrine\\DBAL\\Schema\\Exception\\UnsupportedName";

/// `testAutoIncrementColumnIntrospection(true)`; the `false` case is
/// marked incomplete on `SQLite` (doctrine/dbal#6844).
#[tokio::test]
async fn testAutoIncrementColumnIntrospection() {
    let autoincrement = true;
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    assert!(platform(&connection).await.supports_identity_columns());

    let table = Table::editor()
        .set_unquoted_name("test_autoincrement", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_autoincrement(autoincrement)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table = sm
        .introspect_table_by_unquoted_name("test_autoincrement", None)
        .await
        .unwrap();

    assert_eq!(
        autoincrement,
        table.get_column("id").unwrap().get_autoincrement()
    );
}

/// `testAutoIncrementColumnInCompositePrimaryKeyIntrospection(false)`; the
/// `true` case is skipped on `SQLite`.
#[tokio::test]
async fn testAutoIncrementColumnInCompositePrimaryKeyIntrospection() {
    let autoincrement = false;
    let connection = sqlite();
    let sm = schema_manager(&connection).await;

    let table = Table::editor()
        .set_unquoted_name("test_autoincrement", None)
        .set_columns(vec![
            column("id1", Types::INTEGER)
                .set_autoincrement(autoincrement)
                .create()
                .unwrap(),
            column("id2", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id1", "id2"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table = sm
        .introspect_table_by_unquoted_name("test_autoincrement", None)
        .await
        .unwrap();

    assert_eq!(
        autoincrement,
        table.get_column("id1").unwrap().get_autoincrement()
    );
    assert!(!table.get_column("id2").unwrap().get_autoincrement());
}

#[tokio::test]
async fn testIntrospectTableWithDotInName() {
    for quoted in [false, true] {
        let connection = sqlite();
        let sm = schema_manager(&connection).await;
        let platform = platform(&connection).await;
        assert!(!platform.supports_schemas());

        let name = "example.com";
        // SQLite does not fold unquoted identifiers.
        let normalized_name = name;
        let quoted_name = platform.quote_single_identifier(normalized_name);

        // create the table manually since identifiers with dots are not
        // supported in DBAL 4.x
        let sql = format!("CREATE TABLE {quoted_name} (s VARCHAR(16))");

        drop_table_if_exists(&connection, &quoted_name).await;
        exec(&connection, &sql).await;

        deprecations::enable_tracking_deprecations();
        let before = deprecations::count(LINK_6768);
        let table = if quoted {
            let table = sm.introspect_table(&quoted_name).await.unwrap();
            assert_eq!(before, deprecations::count(LINK_6768));
            table
        } else {
            let table = sm.introspect_table(name).await.unwrap();
            assert!(deprecations::count(LINK_6768) > before);
            table
        };
        deprecations::disable();

        assert_eq!(1, table.get_columns().len(), "{quoted}");
    }
}

#[tokio::test]
async fn testIntrospectTableWithInvalidName() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_quoted_name("example", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(LINK_6768);
    let table = sm.introspect_table("\"example").await.unwrap();
    assert!(deprecations::count(LINK_6768) > before);
    deprecations::disable();

    assert_eq!(1, table.get_columns().len());
}

#[tokio::test]
async fn testChangeColumnNullability() {
    for not_null in [true, false] {
        let connection = sqlite();
        let sm = schema_manager(&connection).await;
        let table = Table::editor()
            .set_unquoted_name("change_column_nullability", None)
            .set_columns(vec![
                column("id", Types::INTEGER).create().unwrap(),
                column("val", Types::INTEGER)
                    .set_not_null(!not_null)
                    .create()
                    .unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        let table = sm
            .introspect_table_by_unquoted_name("change_column_nullability", None)
            .await
            .unwrap();

        let new_table = table
            .edit()
            .unwrap()
            .modify_column_by_unquoted_name("val", |editor| editor.set_not_null(not_null))
            .create()
            .unwrap();

        let diff = sm
            .create_comparator()
            .await
            .unwrap()
            .compare_tables(&table, &new_table)
            .unwrap();
        assert!(!diff.is_empty());

        sm.alter_table(&diff).await.unwrap();

        assert_eq!(
            not_null,
            sm.introspect_table_by_unquoted_name("change_column_nullability", None)
                .await
                .unwrap()
                .get_column("val")
                .unwrap()
                .get_notnull(),
            "{not_null}"
        );
    }
}

#[tokio::test]
async fn testDropForeignKey() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    drop_table_if_exists(&connection, "orders").await;
    drop_table_if_exists(&connection, "articles").await;

    let articles = Table::editor()
        .set_unquoted_name("articles", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let orders = Table::editor()
        .set_unquoted_name("orders", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("article_id", Types::INTEGER).create().unwrap(),
        ])
        .set_foreign_key_constraints(vec![foreign_key(
            Some("articles_fk"),
            &["article_id"],
            "articles",
            &["id"],
        )])
        .create()
        .unwrap();

    sm.create_table(&articles).await.unwrap();
    sm.create_table(&orders).await.unwrap();

    sm.drop_foreign_key("articles_fk", "orders").await.unwrap();

    assert!(
        sm.introspect_table_by_unquoted_name("orders", None)
            .await
            .unwrap()
            .get_foreign_keys()
            .is_empty()
    );
}

#[tokio::test]
async fn testReplaceIndexBackingForeignKey() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    drop_table_if_exists(&connection, "index_replacement_child").await;
    drop_table_if_exists(&connection, "index_replacement_parent").await;

    let parent_table = Table::editor()
        .set_unquoted_name("index_replacement_parent", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    // The unique index spans exactly the referencing column of the foreign
    // key, so no implicit index is created, and it remains the only index
    // covering the constraint.
    let child_table = Table::editor()
        .set_unquoted_name("index_replacement_child", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("parent_id", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_indexes(vec![
            Index::editor()
                .set_unquoted_name("uniq_parent_id")
                .set_unquoted_column_names(&["parent_id"])
                .set_type(IndexType::Unique)
                .create()
                .unwrap(),
        ])
        .set_foreign_key_constraints(vec![foreign_key(
            Some("fk_index_replacement"),
            &["parent_id"],
            "index_replacement_parent",
            &["id"],
        )])
        .create()
        .unwrap();

    sm.create_table(&parent_table).await.unwrap();
    sm.create_table(&child_table).await.unwrap();

    let old_table = sm
        .introspect_table_by_unquoted_name("index_replacement_child", None)
        .await
        .unwrap();

    let new_index = index("idx_parent_id", &["parent_id"]);

    let new_table = old_table
        .edit()
        .unwrap()
        .set_indexes(vec![new_index.clone()])
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(&old_table, &new_table)
        .unwrap();
    sm.alter_table(&diff).await.unwrap();

    let table = sm
        .introspect_table_by_unquoted_name("index_replacement_child", None)
        .await
        .unwrap();
    assert!(!table.has_index("uniq_parent_id"));
    assert_index_equals(&new_index, table.get_index("idx_parent_id").unwrap());
}

/// `createTableWithIndex($tableName, $indexName)`.
async fn create_table_with_index(
    connection: &Connection,
    table_name: OptionallyQualifiedName,
    index_name: UnqualifiedName,
) {
    let platform = platform(connection).await;

    let table = Table::editor()
        .set_name(table_name)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_indexes(vec![
            Index::editor()
                .set_name(Some(index_name))
                .set_unquoted_column_names(&["id"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_table_if_exists(
        connection,
        &table.get_object_name().unwrap().to_sql(platform),
    )
    .await;
    schema_manager(connection)
        .await
        .create_table(&table)
        .await
        .unwrap();
}

#[tokio::test]
async fn testIntrospectTableIndexes() {
    // quotedAndUnquotedIndexIntrospection()
    for quoted in [false, true] {
        let connection = sqlite();
        let sm = schema_manager(&connection).await;
        let (table_name, index_name) = if quoted {
            (
                OptionallyQualifiedName::quoted("Orders", None).unwrap(),
                UnqualifiedName::quoted("Orders_index").unwrap(),
            )
        } else {
            (
                OptionallyQualifiedName::unquoted("Orders", None).unwrap(),
                UnqualifiedName::unquoted("Orders_index").unwrap(),
            )
        };
        create_table_with_index(&connection, table_name, index_name).await;

        let indexes = if quoted {
            sm.introspect_table_indexes_by_quoted_name("Orders", None)
                .await
                .unwrap()
        } else {
            sm.introspect_table_indexes_by_unquoted_name("Orders", None)
                .await
                .unwrap()
        };
        assert!(!indexes.is_empty(), "{quoted}");
    }
}

/// `toQuotedUniqueConstraint($constraint)`.
fn to_quoted_unique_constraint(constraint: &UniqueConstraint) -> UniqueConstraint {
    constraint
        .edit()
        .unwrap()
        .set_name(
            constraint
                .get_object_name()
                .unwrap()
                .as_ref()
                .map(to_quoted_unqualified_name),
        )
        .set_column_names(
            constraint
                .get_column_names()
                .unwrap()
                .iter()
                .map(to_quoted_unqualified_name)
                .collect(),
        )
        .create()
        .unwrap()
}

/// `assertUniqueConstraintListEquals($expected, $actual)`.
fn assert_unique_constraint_list_equals(
    expected: &[&UniqueConstraint],
    actual: &[&UniqueConstraint],
) {
    let quoted = |list: &[&UniqueConstraint]| -> Vec<UniqueConstraint> {
        list.iter()
            .map(|c| to_quoted_unique_constraint(c))
            .collect()
    };
    assert_eq!(quoted(expected), quoted(actual));
}

/// `createTableWithUniqueConstraint($tableName, $uniqueConstraint)`.
async fn create_table_with_unique_constraint(
    connection: &Connection,
    table_name: OptionallyQualifiedName,
    unique_constraint: &UniqueConstraint,
) {
    let platform = platform(connection).await;

    let columns = unique_constraint
        .get_column_names()
        .unwrap()
        .iter()
        .map(|column_name| {
            Column::editor()
                .set_name(column_name.clone())
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap()
        })
        .collect();

    let table = Table::editor()
        .set_name(table_name)
        .set_columns(columns)
        .set_unique_constraints(vec![unique_constraint.clone()])
        .create()
        .unwrap();

    drop_table_if_exists(
        connection,
        &table.get_object_name().unwrap().to_sql(platform),
    )
    .await;
    schema_manager(connection)
        .await
        .create_table(&table)
        .await
        .unwrap();
}

#[tokio::test]
async fn testIntrospectTableUniqueConstraints() {
    // quotedAndUnquotedUniqueConstraintIntrospection()
    for quoted in [false, true] {
        let connection = sqlite();
        let sm = schema_manager(&connection).await;
        let (table_name, unique_constraint) = if quoted {
            (
                OptionallyQualifiedName::quoted("Enrollments", None).unwrap(),
                UniqueConstraint::editor()
                    .set_quoted_name("UQ_StudentCourse")
                    .set_quoted_column_names(&["StudentId", "CourseId"])
                    .create()
                    .unwrap(),
            )
        } else {
            (
                OptionallyQualifiedName::unquoted("Enrollments", None).unwrap(),
                UniqueConstraint::editor()
                    .set_unquoted_name("UQ_StudentCourse")
                    .set_unquoted_column_names(&["StudentId", "CourseId"])
                    .create()
                    .unwrap(),
            )
        };
        create_table_with_unique_constraint(&connection, table_name, &unique_constraint).await;

        let actual = if quoted {
            sm.introspect_table_unique_constraints_by_quoted_name("Enrollments", None)
                .await
                .unwrap()
        } else {
            sm.introspect_table_unique_constraints_by_unquoted_name("Enrollments", None)
                .await
                .unwrap()
        };
        assert_unique_constraint_list_equals(
            &[&unique_constraint],
            &actual.iter().collect::<Vec<_>>(),
        );
    }
}

/// `createUniqueConstraint()`.
fn create_unique_constraint() -> UniqueConstraint {
    UniqueConstraint::editor()
        .set_unquoted_name("UQ_StudentCourse")
        .set_column_names(vec![
            UnqualifiedName::unquoted("StudentId").unwrap(),
            UnqualifiedName::quoted("CourseId").unwrap(),
        ])
        .create()
        .unwrap()
}

/// `findTable($tables, $name)`: `SQLite` does not fold.
fn find_table<'t>(tables: &'t [Table], name: &OptionallyQualifiedName) -> Option<&'t Table> {
    let expected_name = name.get_unqualified_name().get_value();
    tables.iter().find(|table| {
        table
            .get_object_name()
            .unwrap()
            .get_unqualified_name()
            .get_value()
            == expected_name
    })
}

#[tokio::test]
async fn testIntrospectAllTablesUniqueConstraintsOnSchemalessPlatform() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    assert!(!platform(&connection).await.supports_schemas());

    let unique_constraint = create_unique_constraint();
    let table_name = OptionallyQualifiedName::unquoted("Enrollments", None).unwrap();

    create_table_with_unique_constraint(&connection, table_name.clone(), &unique_constraint).await;

    let tables = sm.introspect_tables().await.unwrap();
    let table = find_table(&tables, &table_name);
    assert!(table.is_some());

    assert_unique_constraint_list_equals(
        &[&unique_constraint],
        &table
            .unwrap()
            .get_unique_constraints()
            .values()
            .collect::<Vec<_>>(),
    );
}

/// `createTablesWithForeignKey(...)`.
async fn create_tables_with_foreign_key(
    connection: &Connection,
    referenced_table_name: OptionallyQualifiedName,
    referencing_table_name: OptionallyQualifiedName,
    index_name: UnqualifiedName,
    foreign_key_name: UnqualifiedName,
) {
    let platform = platform(connection).await;
    let sm = schema_manager(connection).await;

    let referenced_table = Table::editor()
        .set_name(referenced_table_name.clone())
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let referencing_table = Table::editor()
        .set_name(referencing_table_name)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("article_id", Types::INTEGER).create().unwrap(),
        ])
        // Create the index explicitly to prevent the implicit one, whose
        // name is auto-generated.
        .set_indexes(vec![
            Index::editor()
                .set_name(Some(index_name))
                .set_unquoted_column_names(&["article_id"])
                .create()
                .unwrap(),
        ])
        .set_foreign_key_constraints(vec![
            ForeignKeyConstraint::editor()
                .set_name(Some(foreign_key_name))
                .set_unquoted_referencing_column_names(&["article_id"])
                .set_referenced_table_name(referenced_table_name)
                .set_unquoted_referenced_column_names(&["id"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_table_if_exists(
        connection,
        &referencing_table
            .get_object_name()
            .unwrap()
            .to_sql(platform),
    )
    .await;
    drop_table_if_exists(
        connection,
        &referenced_table.get_object_name().unwrap().to_sql(platform),
    )
    .await;

    sm.create_table(&referenced_table).await.unwrap();
    sm.create_table(&referencing_table).await.unwrap();
}

#[tokio::test]
async fn testIntrospectTableForeignKeyConstraints() {
    // quotedAndUnquotedForeignKeyIntrospection()
    for quoted in [false, true] {
        let connection = sqlite();
        let sm = schema_manager(&connection).await;
        let optionally_qualified = |name: &str| {
            if quoted {
                OptionallyQualifiedName::quoted(name, None).unwrap()
            } else {
                OptionallyQualifiedName::unquoted(name, None).unwrap()
            }
        };
        let unqualified = |name: &str| {
            if quoted {
                UnqualifiedName::quoted(name).unwrap()
            } else {
                UnqualifiedName::unquoted(name).unwrap()
            }
        };
        create_tables_with_foreign_key(
            &connection,
            optionally_qualified("Articles"),
            optionally_qualified("Orders"),
            unqualified("Orders_article_id_index"),
            unqualified("Articles_fk"),
        )
        .await;

        let foreign_keys = if quoted {
            sm.introspect_table_foreign_key_constraints_by_quoted_name("Orders", None)
                .await
                .unwrap()
        } else {
            sm.introspect_table_foreign_key_constraints_by_unquoted_name("Orders", None)
                .await
                .unwrap()
        };
        assert!(!foreign_keys.is_empty(), "{quoted}");
    }
}

#[tokio::test]
async fn testIntrospectSchemaNamesOnSchemalessPlatform() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;

    let e = sm.introspect_schema_names().await.unwrap_err();
    assert!(e.is_instance_of(NOT_SUPPORTED), "{e}");
}

#[tokio::test]
async fn testIntrospectSequencesWithoutSequenceSupport() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;

    let e = sm.introspect_sequences().await.unwrap_err();
    assert!(e.is_instance_of(NOT_SUPPORTED), "{e}");
}

#[tokio::test]
async fn testCreateSequenceWithoutSequenceSupport() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;

    let e = sm
        .create_sequence(
            &Sequence::editor()
                .set_unquoted_name("s", None)
                .create()
                .unwrap(),
        )
        .await
        .unwrap_err();
    assert!(e.is_instance_of(NOT_SUPPORTED), "{e}");
}

#[tokio::test]
async fn testIntrospectionWithSchemaNameWithoutSchemaSupport() {
    let connection = sqlite();
    let platform = SQLitePlatform::new();
    assert!(!platform.supports_schemas());
    let provider = platform.create_metadata_provider(&connection);

    // perTableIntrospectionProvider()
    let check = |case: &str, result: Result<usize, Exception>| {
        let e = result.unwrap_err();
        assert!(e.is_instance_of(UNSUPPORTED_NAME), "{case}: {e}");
    };
    let (schema_name, table_name) = (Some("billing"), "orders");

    check(
        "columns",
        provider
            .get_table_columns_for_table(schema_name, table_name)
            .await
            .map(|rows| rows.len()),
    );
    check(
        "indexes",
        provider
            .get_index_columns_for_table(schema_name, table_name)
            .await
            .map(|rows| rows.len()),
    );
    check(
        "primary key constraint",
        provider
            .get_primary_key_constraint_columns_for_table(schema_name, table_name)
            .await
            .map(|rows| rows.len()),
    );
    check(
        "unique constraints",
        provider
            .get_unique_constraint_columns_for_table(schema_name, table_name)
            .await
            .unwrap()
            .map(|rows| rows.len()),
    );
    check(
        "foreign key constraints",
        provider
            .get_foreign_key_constraint_columns_for_table(schema_name, table_name)
            .await
            .map(|rows| rows.len()),
    );
    check(
        "options",
        provider
            .get_table_options_for_table(schema_name, table_name)
            .await
            .map(|rows| rows.len()),
    );
}
