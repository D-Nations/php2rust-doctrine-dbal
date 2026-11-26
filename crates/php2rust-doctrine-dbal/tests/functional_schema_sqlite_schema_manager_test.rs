//! Port of tests/Functional/Schema/SQLiteSchemaManagerTest.php and the
//! tests it inherits from
//! tests/Functional/Schema/SchemaManagerFunctionalTestCase.php, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's; a data
//! provider's cases run in one test, in order. `assertEquals()` of
//! foreign keys compares their names, columns and options. The tests
//! that upstream skips on `SQLite` are not here (parity marks them n/a).
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/SQLiteSchemaManagerTest.php
//! Upstream: doctrine/dbal tests/Functional/Schema/SchemaManagerFunctionalTestCase.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]
#![allow(clippy::similar_names, reason = "the names are upstream's")]

mod schema_manager_support;

use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::deprecations;
use php2rust_doctrine_dbal::schema::name::UnqualifiedName;
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, ColumnDiff, ComparatorConfig, ForeignKeyConstraint, Index, IndexType,
    Schema, Table, TableDiff, TableDiffParts, View,
};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Configuration, Connection, ParameterTypes};
use schema_manager_support::{
    assert_foreign_key_constraint_list_equals, assert_foreign_key_equals, assert_index_equals,
    assert_index_list_equals, assert_unqualified_name_equals, column, create_test_table,
    drop_and_create_table, drop_table_if_exists, exec, find_table_by_shortest_name, foreign_key,
    get_test_composite_table, get_test_table, index, lower, platform, primary_key, schema_manager,
    sqlite, sqlite_with,
};
use std::sync::Arc;

const LINK_6701: &str = "https://github.com/doctrine/dbal/pull/6701";

fn fk_options(on_update: &str, on_delete: &str, deferrable: bool, deferred: bool) -> PhpArray {
    let mut options = PhpArray::new();
    options.insert("onUpdate", Value::string(on_update));
    options.insert("onDelete", Value::string(on_delete));
    options.insert("deferrable", Value::Bool(deferrable));
    options.insert("deferred", Value::Bool(deferred));
    options
}

/// SQLITE does not support databases.
#[tokio::test]
async fn testIntrospectDatabaseNames() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;

    let e = sm.introspect_database_names().await.unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

#[tokio::test]
async fn testListForeignKeysFromExistingDatabase() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    exec(&connection, "DROP TABLE IF EXISTS user").await;
    exec(
        &connection,
        "CREATE TABLE user (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page INTEGER CONSTRAINT FK_1 REFERENCES page (key) DEFERRABLE INITIALLY DEFERRED,
    parent INTEGER REFERENCES user(id) ON DELETE CASCADE
)",
    )
    .await;

    let expected = [
        ForeignKeyConstraint::new(
            ["page"],
            "page",
            ["key"],
            "FK_1",
            fk_options("NO ACTION", "NO ACTION", true, true),
        )
        .unwrap(),
        ForeignKeyConstraint::new(
            ["parent"],
            "user",
            ["id"],
            "",
            fk_options("NO ACTION", "CASCADE", false, false),
        )
        .unwrap(),
    ];

    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(LINK_6701);
    let actual = sm.list_table_foreign_keys("user").await.unwrap();
    assert_eq!(before, deprecations::count(LINK_6701));
    deprecations::disable();

    assert_eq!(expected.len(), actual.len());
    for (expected, actual) in expected.iter().zip(&actual) {
        assert_foreign_key_equals(expected, actual);
    }
}

#[tokio::test]
async fn testListForeignKeysWithImplicitColumnsFromIncompleteSchema() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    exec(&connection, "DROP TABLE IF EXISTS t1").await;
    exec(
        &connection,
        "CREATE TABLE t1 (
    id INTEGER,
    t2_id INTEGER,
    FOREIGN KEY (t2_id) REFERENCES t2
)",
    )
    .await;

    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(LINK_6701);

    let expected = ForeignKeyConstraint::new(
        ["t2_id"],
        "t2",
        Vec::<String>::new(),
        "",
        fk_options("NO ACTION", "NO ACTION", false, false),
    )
    .unwrap();

    let actual = sm.list_table_foreign_keys("t1").await.unwrap();
    assert!(deprecations::count(LINK_6701) > before);
    deprecations::disable();

    assert_eq!(1, actual.len());
    assert_foreign_key_equals(&expected, &actual[0]);
    exec(&connection, "DROP TABLE t1").await;
}

#[tokio::test]
async fn testColumnCollation() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_collation", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("text", Types::TEXT).create().unwrap(),
            column("foo", Types::TEXT)
                .set_collation(Some("BINARY"))
                .create()
                .unwrap(),
            column("bar", Types::TEXT)
                .set_collation(Some("NOCASE"))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_collation", None)
        .await
        .unwrap();

    assert_eq!(None, columns[0].get_collation());
    assert_eq!(Some("BINARY".to_owned()), columns[1].get_collation());
    assert_eq!(Some("BINARY".to_owned()), columns[2].get_collation());
    assert_eq!(Some("NOCASE".to_owned()), columns[3].get_collation());
}

#[tokio::test]
async fn testListTableColumnsWithWhitespacesInTypeDeclarations() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    exec(
        &connection,
        "CREATE TABLE dbal_1779 (
    foo VARCHAR (64) ,
    bar TEXT (100)
)",
    )
    .await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("dbal_1779", None)
        .await
        .unwrap();

    assert_eq!(2, columns.len());
    let (foo, bar) = (&columns[0], &columns[1]);

    assert_eq!(Types::STRING, foo.get_type_name());
    assert_eq!(Types::TEXT, bar.get_type_name());

    assert_eq!(Some(64), foo.get_length());
    assert_eq!(Some(100), bar.get_length());
}

#[tokio::test]
async fn testListTableColumnsWithMixedCaseInTypeDeclarations() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    exec(
        &connection,
        "CREATE TABLE dbal_mixed (
    foo VarChar (64),
    bar Text (100)
)",
    )
    .await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("dbal_mixed", None)
        .await
        .unwrap();

    assert_eq!(2, columns.len());
    assert_eq!(Types::STRING, columns[0].get_type_name());
    assert_eq!(Types::TEXT, columns[1].get_type_name());
}

/// `SQLiteSchemaManagerTest`'s own `testPrimaryKeyAutoIncrement()`, which
/// overrides the parent's.
#[tokio::test]
async fn testPrimaryKeyAutoIncrement() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("test_pk_auto_increment", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("text", Types::TEXT).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let mut data = PhpArray::new();
    data.insert("text", Value::string("1"));
    connection
        .insert("test_pk_auto_increment", &data, &ParameterTypes::new())
        .await
        .unwrap();

    exec(&connection, "DELETE FROM test_pk_auto_increment").await;

    let mut data = PhpArray::new();
    data.insert("text", Value::string("2"));
    connection
        .insert("test_pk_auto_increment", &data, &ParameterTypes::new())
        .await
        .unwrap();

    let last_used_id_after_delete = connection
        .fetch_one(
            "SELECT id FROM test_pk_auto_increment WHERE text = \"2\"",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap()
        .unwrap();

    // with an empty table, non autoincrement rowid is always 1
    assert!(php_value::vectors::phpunit_equals(
        &Value::Int(1),
        &last_used_id_after_delete
    ));
}

#[tokio::test]
async fn testOnlyOwnCommentIsParsed() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("own_column_comment", None)
        .set_columns(vec![
            column("col1", Types::STRING)
                .set_length(Some(16))
                .create()
                .unwrap(),
            column("col2", Types::STRING)
                .set_length(Some(16))
                .set_comment("Column #2")
                .create()
                .unwrap(),
            column("col3", Types::STRING)
                .set_length(Some(16))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let sm = connection.create_schema_manager().await.unwrap();
    sm.create_table(&table).await.unwrap();

    assert_eq!(
        "",
        sm.introspect_table_by_unquoted_name("own_column_comment", None)
            .await
            .unwrap()
            .get_column("col1")
            .unwrap()
            .get_comment()
    );
}

#[tokio::test]
async fn testIntrospectWithoutRowidTableOption() {
    for without_rowid in [true, false] {
        let connection = sqlite();
        let sm = schema_manager(&connection).await;
        let mut options = PhpArray::new();
        options.insert("without_rowid", Value::Bool(without_rowid));
        let table = Table::editor()
            .set_unquoted_name("dbal_without_rowid", None)
            .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
            .set_primary_key_constraint(primary_key(&["id"]))
            .set_options(options)
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        let online_table = sm
            .introspect_table_by_unquoted_name("dbal_without_rowid", None)
            .await
            .unwrap();

        assert!(
            Value::Bool(without_rowid).identical(&online_table.get_option("without_rowid")),
            "{without_rowid}: {:?}",
            online_table.get_option("without_rowid")
        );
    }
}

#[tokio::test]
async fn testIntrospectWithoutRowidTableOptionFollowedByAnotherOption() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    drop_table_if_exists(&connection, "dbal_without_rowid_strict").await;

    exec(
        &connection,
        "CREATE TABLE dbal_without_rowid_strict (
    id INTEGER NOT NULL,
    PRIMARY KEY (id)
) WITHOUT ROWID, STRICT",
    )
    .await;

    let online_table = sm
        .introspect_table_by_unquoted_name("dbal_without_rowid_strict", None)
        .await
        .unwrap();

    assert!(Value::Bool(true).identical(&online_table.get_option("without_rowid")));
}

#[tokio::test]
async fn testIntrospectColumnNamedLikeWithoutRowidTableOption() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    drop_table_if_exists(&connection, "dbal_without_rowid_column").await;

    exec(
        &connection,
        "CREATE TABLE dbal_without_rowid_column (
    id INTEGER NOT NULL,
    \"without rowid\" TEXT
)",
    )
    .await;

    let online_table = sm
        .introspect_table_by_unquoted_name("dbal_without_rowid_column", None)
        .await
        .unwrap();

    assert!(Value::Bool(false).identical(&online_table.get_option("without_rowid")));
}

#[tokio::test]
async fn testNonSimpleAlterTableCreatedFromDDL() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "nodes").await;

    exec(
        &connection,
        "CREATE TABLE nodes (
    id        INTEGER NOT NULL,
    parent_id INTEGER,
    name      TEXT,
    PRIMARY KEY (id),
    FOREIGN KEY (parent_id) REFERENCES nodes (id)
)",
    )
    .await;

    let schema_manager = connection.create_schema_manager().await.unwrap();

    let table1 = schema_manager.introspect_table("nodes").await.unwrap();
    let table2 = table1
        .edit()
        .unwrap()
        .add_index(schema_manager_support::index("idx_name", &["name"]))
        .create()
        .unwrap();

    let comparator = schema_manager.create_comparator();
    let diff = comparator.compare_tables(&table1, &table2).unwrap();

    schema_manager.alter_table(&diff).await.unwrap();

    let table = schema_manager.introspect_table("nodes").await.unwrap();
    let index = table.get_index("idx_name").unwrap();
    assert_eq!(vec!["name".to_owned()], index.get_columns());
}

#[tokio::test]
async fn testAlterTableWithSchema() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    drop_table_if_exists(&connection, "t").await;

    let table = Table::editor()
        .set_unquoted_name("t", Some("main"))
        .set_columns(vec![column("a", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    sm.create_table(&table).await.unwrap();

    let columns = sm
        .introspect_table_columns_by_unquoted_name("t", None)
        .await
        .unwrap();

    assert_unqualified_name_equals("a", &columns[0].get_object_name().unwrap());

    let table_diff = TableDiff::new(
        table,
        TableDiffParts {
            changed_columns: vec![ColumnDiff::new(
                column("a", Types::INTEGER).create().unwrap(),
                column("b", Types::INTEGER).create().unwrap(),
            )],
            ..TableDiffParts::default()
        },
    );
    sm.alter_table(&table_diff).await.unwrap();

    let columns = sm
        .introspect_table_columns_by_unquoted_name("t", None)
        .await
        .unwrap();

    assert_unqualified_name_equals("b", &columns[0].get_object_name().unwrap());
}

#[tokio::test]
async fn testIntrospectMultipleAnonymousForeignKeyConstraints() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "album").await;
    drop_table_if_exists(&connection, "song").await;

    exec(
        &connection,
        "CREATE TABLE artist(
  id INTEGER,
  name TEXT,
  PRIMARY KEY(id)
);

CREATE TABLE album(
  id INTEGER,
  name TEXT,
  PRIMARY KEY(id)
);

CREATE TABLE song(
  id     INTEGER,
  album_id INTEGER,
  artist_id INTEGER,
  FOREIGN KEY(album_id) REFERENCES album(id),
  FOREIGN KEY(artist_id) REFERENCES artist(id)
);",
    )
    .await;

    let schema_manager = connection.create_schema_manager().await.unwrap();

    let song = schema_manager.introspect_table("song").await.unwrap();

    let foreign_keys: Vec<&ForeignKeyConstraint> = song.get_foreign_keys().values().collect();
    assert_eq!(2, foreign_keys.len());

    let foreign_key1 = foreign_keys[0];
    assert!(foreign_key1.get_object_name().unwrap().is_none());

    assert_eq!(
        vec!["album_id".to_owned()],
        foreign_key1.get_local_columns()
    );
    assert_eq!(vec!["id".to_owned()], foreign_key1.get_foreign_columns());

    let foreign_key2 = foreign_keys[1];
    assert!(foreign_key2.get_object_name().unwrap().is_none());

    assert_eq!(
        vec!["artist_id".to_owned()],
        foreign_key2.get_local_columns()
    );
    assert_eq!(vec!["id".to_owned()], foreign_key2.get_foreign_columns());
}

#[tokio::test]
async fn testIntrospectMultipleAnonymousUniqueConstraints() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    drop_table_if_exists(&connection, "t").await;

    exec(
        &connection,
        "CREATE TABLE t (
  a INTEGER,
  b INTEGER,
  UNIQUE (a),
  UNIQUE (b)
)",
    )
    .await;

    let table = sm
        .introspect_table_by_unquoted_name("t", None)
        .await
        .unwrap();

    let unique_constraints: Vec<_> = table.get_unique_constraints().values().collect();
    assert_eq!(2, unique_constraints.len());

    for unique_constraint in unique_constraints {
        assert!(unique_constraint.get_object_name().unwrap().is_none());
    }
}

#[tokio::test]
async fn testIntrospectMultipleNamedUniqueConstraints() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    drop_table_if_exists(&connection, "t").await;

    // SQLite does not report a unique constraint's declared name; it must
    // be reconstructed and matched to the right constraint by declaration
    // order.
    exec(
        &connection,
        "CREATE TABLE t (
  a INTEGER,
  b INTEGER,
  c INTEGER,
  d INTEGER,
  PRIMARY KEY (a, b),
  CONSTRAINT uq_c UNIQUE (c),
  CONSTRAINT uq_d UNIQUE (d)
)",
    )
    .await;

    let table = sm
        .introspect_table_by_unquoted_name("t", None)
        .await
        .unwrap();

    let mut names_by_column = Vec::new();
    for unique_constraint in table.get_unique_constraints().values() {
        let name = unique_constraint.get_object_name().unwrap();
        assert!(name.is_some());

        let columns: Vec<String> = unique_constraint
            .get_column_names()
            .unwrap()
            .iter()
            .map(|column| column.get_identifier().get_value().to_owned())
            .collect();
        assert_eq!(1, columns.len());

        names_by_column.push((
            columns[0].clone(),
            name.unwrap().get_identifier().get_value().to_owned(),
        ));
    }

    assert_eq!(
        vec![
            ("c".to_owned(), "uq_c".to_owned()),
            ("d".to_owned(), "uq_d".to_owned())
        ],
        names_by_column
    );
}

#[tokio::test]
async fn testNoWhitespaceInForeignKeyReference() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    drop_table_if_exists(&connection, "notes").await;
    drop_table_if_exists(&connection, "users").await;

    exec(
        &connection,
        "CREATE TABLE \"users\" (
    \"id\" INTEGER
);

CREATE TABLE \"notes\" (
    \"id\" INTEGER,
    \"created_by\" INTEGER,
    FOREIGN KEY(\"created_by\") REFERENCES \"users\"(\"id\"));",
    )
    .await;
    let notes = sm.introspect_table("notes").await.unwrap();

    let foreign_keys: Vec<&ForeignKeyConstraint> = notes.get_foreign_keys().values().collect();
    assert_eq!(1, foreign_keys.len());

    let foreign_key = foreign_keys[0];

    assert_eq!(
        vec!["created_by".to_owned()],
        foreign_key.get_local_columns()
    );
    assert_eq!("users", foreign_key.get_foreign_table_name());
    assert_eq!(vec!["id".to_owned()], foreign_key.get_foreign_columns());
}

#[tokio::test]
async fn testShorthandInForeignKeyReference() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "artist").await;
    drop_table_if_exists(&connection, "track").await;

    exec(
        &connection,
        "CREATE TABLE artist(
    artistid INTEGER PRIMARY KEY,
    artistname TEXT
);

CREATE TABLE track(
    trackid INTEGER,
    trackname TEXT,
    trackartist INTEGER REFERENCES artist
);",
    )
    .await;

    let schema_manager = connection.create_schema_manager().await.unwrap();

    let song = schema_manager.introspect_table("track").await.unwrap();

    let foreign_keys: Vec<&ForeignKeyConstraint> = song.get_foreign_keys().values().collect();
    assert_eq!(1, foreign_keys.len());

    let foreign_key1 = foreign_keys[0];
    assert!(foreign_key1.get_object_name().unwrap().is_none());

    assert_eq!(
        vec!["trackartist".to_owned()],
        foreign_key1.get_local_columns()
    );
    assert_eq!(
        vec!["artistid".to_owned()],
        foreign_key1.get_foreign_columns()
    );
}

#[tokio::test]
async fn testShorthandInForeignKeyReferenceWithMultipleColumns() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "artist").await;
    drop_table_if_exists(&connection, "track").await;

    exec(
        &connection,
        "CREATE TABLE artist(
    artistid INTEGER,
    isrc TEXT,
    artistname TEXT,
    PRIMARY KEY (artistid, isrc)
);

CREATE TABLE track(
    trackid INTEGER,
    trackname TEXT,
    trackartist INTEGER REFERENCES artist
);",
    )
    .await;

    let schema_manager = connection.create_schema_manager().await.unwrap();

    let track = schema_manager.introspect_table("track").await.unwrap();
    let foreign_keys: Vec<&ForeignKeyConstraint> = track.get_foreign_keys().values().collect();
    assert_eq!(1, foreign_keys.len());

    let foreign_key1 = foreign_keys[0];
    assert!(foreign_key1.get_name().is_empty());

    assert_eq!(
        vec!["trackartist".to_owned()],
        foreign_key1.get_local_columns()
    );
    assert_eq!(
        vec!["artistid".to_owned(), "isrc".to_owned()],
        foreign_key1.get_foreign_columns()
    );

    let create_table_track_sql = platform(&connection)
        .await
        .get_create_table_sql(&track)
        .unwrap();

    assert_eq!(
        vec![
            "CREATE TABLE track (trackid INTEGER DEFAULT NULL, trackname CLOB DEFAULT NULL COLLATE \"BINARY\", trackartist INTEGER DEFAULT NULL, FOREIGN KEY (trackartist) REFERENCES artist (artistid, isrc) ON UPDATE NO ACTION ON DELETE NO ACTION NOT DEFERRABLE INITIALLY IMMEDIATE)".to_owned(),
            "CREATE INDEX IDX_D6E3F8A6FB96D8BC ON track (trackartist)".to_owned(),
        ],
        create_table_track_sql
    );
}

#[tokio::test]
async fn testIntrospectPrimaryKeyAutoincrement() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "dbal_7271_1").await;
    drop_table_if_exists(&connection, "dbal_7271_2").await;
    drop_table_if_exists(&connection, "dbal_7271_3").await;

    exec(
        &connection,
        "CREATE TABLE dbal_7271_1 (
    user_id   INTEGER,
    user_name VARCHAR,
    PRIMARY KEY (user_id)
);
CREATE TABLE dbal_7271_2 (
    user_id       INTEGER,
    setting_name  VARCHAR,
    setting_value TEXT,
    PRIMARY KEY (user_id, setting_name)
);
CREATE TABLE dbal_7271_3 (
    setting_name  VARCHAR,
    setting_value VARCHAR,
    PRIMARY KEY (setting_name)
);",
    )
    .await;

    let schema_manager = connection.create_schema_manager().await.unwrap();

    let autoincrement = |table: &'static str, column: &'static str| {
        let schema_manager = &schema_manager;
        async move {
            schema_manager
                .introspect_schema()
                .await
                .unwrap()
                .get_table(table)
                .unwrap()
                .get_column(column)
                .unwrap()
                .get_autoincrement()
        }
    };

    let autoincrement1 = autoincrement("dbal_7271_1", "user_id").await;
    let autoincrement2 = autoincrement("dbal_7271_2", "user_id").await;
    let autoincrement3 = autoincrement("dbal_7271_3", "setting_name").await;

    assert!(autoincrement1, "Integer PKs are implicitly autoincrement");
    assert!(!autoincrement2, "Composite PKs cannot autoincrement");
    assert!(!autoincrement3, "Non-integer PKs cannot autoincrement");
}

#[tokio::test]
async fn testListTableNoSchemaEmulation() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "`list_table_no_schema_emulation.test`").await;

    exec(
        &connection,
        "CREATE TABLE `list_table_no_schema_emulation.test` (
    id INTEGER,
    parent_id INTEGER,
    PRIMARY KEY (id),
    FOREIGN KEY (parent_id) REFERENCES `list_table_no_schema_emulation.test` (id)
);",
    )
    .await;

    exec(
        &connection,
        "CREATE INDEX i ON `list_table_no_schema_emulation.test` (parent_id);",
    )
    .await;

    // The anonymous subclass calls the protected select*() methods, which
    // the trait makes public.
    let custom_sqlite_schema_manager = schema_manager(&connection).await;

    let pairs = |rows: Vec<PhpArray>, keys: &[&str]| -> Vec<Vec<String>> {
        rows.iter()
            .map(|row| {
                keys.iter()
                    .map(|key| {
                        row.get(&Key::from(*key))
                            .and_then(Value::to_php_string)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect()
            })
            .collect()
    };

    let name = "list_table_no_schema_emulation.test".to_owned();

    assert_eq!(
        vec![
            vec![name.clone(), "id".to_owned()],
            vec![name.clone(), "parent_id".to_owned()],
        ],
        pairs(
            custom_sqlite_schema_manager
                .select_table_columns("main", Some(&name))
                .await
                .unwrap()
                .fetch_all_associative()
                .unwrap(),
            &["table_name", "name"]
        )
    );

    assert_eq!(
        vec![vec![name.clone(), "i".to_owned()]],
        pairs(
            custom_sqlite_schema_manager
                .select_index_columns("main", Some(&name))
                .await
                .unwrap()
                .fetch_all_associative()
                .unwrap(),
            &["table_name", "name"]
        )
    );

    assert_eq!(
        vec![vec![name.clone(), "parent_id".to_owned(), "id".to_owned()]],
        pairs(
            custom_sqlite_schema_manager
                .select_foreign_key_columns("main", Some(&name))
                .await
                .unwrap()
                .fetch_all_associative()
                .unwrap(),
            &["table_name", "from", "to"]
        )
    );
}

/// This test duplicates `testCommentInTable()` with the only difference
/// that the name of the table being created is quoted.
#[tokio::test]
async fn testCommentInQuotedTable() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_quoted_name("table_with_comment", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_comment("This is a comment")
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table = sm
        .introspect_table_by_unquoted_name("table_with_comment", None)
        .await
        .unwrap();
    assert_eq!(Some("This is a comment".to_owned()), table.get_comment());
}

/// `getDataColumnCollation()`.
const DATA_COLUMN_COLLATION: &[(&str, &str, &str)] = &[
    (
        r#"CREATE TABLE "test_collation" ("a" text DEFAULT 'aa' COLLATE "RTRIM" NOT NULL)"#,
        "a",
        "RTRIM",
    ),
    (
        r#"CREATE TABLE "test_collation" (
    "b" text UNIQUE NOT NULL COLLATE NOCASE,
    "a" text DEFAULT 'aa' COLLATE "RTRIM" NOT NULL
)"#,
        "a",
        "RTRIM",
    ),
    (
        r#"CREATE TABLE "test_collation" (
    "a" TEXT DEFAULT (LOWER(LTRIM(' a') || RTRIM('a '))) CHECK ("a") NOT NULL COLLATE NOCASE UNIQUE,
    "b" TEXT COLLATE RTRIM
)"#,
        "a",
        "NOCASE",
    ),
    (
        r#"CREATE TABLE "test_collation" ("a" text CHECK ("a") NOT NULL, "b" text COLLATE RTRIM)"#,
        "a",
        "BINARY",
    ),
    (
        r#"CREATE TABLE "test_collation" ("a""b" text COLLATE RTRIM)"#,
        "a\"b",
        "RTRIM",
    ),
    (
        r#"CREATE TABLE "test_collation" (bb TEXT COLLATE RTRIM, b VARCHAR(42) NOT NULL COLLATE BINARY)"#,
        "b",
        "BINARY",
    ),
    (
        r#"CREATE TABLE "test_collation" (
    bbb TEXT COLLATE NOCASE,
    bb TEXT COLLATE RTRIM,
    b VARCHAR(42) NOT NULL COLLATE BINARY
)"#,
        "b",
        "BINARY",
    ),
    (
        r#"CREATE TABLE "test_collation" (b VARCHAR(42) NOT NULL COLLATE BINARY, bb TEXT COLLATE RTRIM)"#,
        "b",
        "BINARY",
    ),
    (
        r#"CREATE TABLE test_collation (
    id INTEGER NOT NULL,
    foo VARCHAR(255) NOT NULL,
    "bar#" VARCHAR(255) COLLATE "NOCASE" NOT NULL,
    baz VARCHAR(255) NOT NULL,
    PRIMARY KEY (id)
)"#,
        "bar#",
        "NOCASE",
    ),
    (
        r#"CREATE TABLE test_collation (
    id INTEGER NOT NULL,
    foo VARCHAR(255) NOT NULL,
    "bar#" VARCHAR(255) NOT NULL,
    baz VARCHAR(255) NOT NULL,
    PRIMARY KEY (id)
)"#,
        "bar#",
        "BINARY",
    ),
    (
        r#"CREATE TABLE test_collation (id INTEGER NOT NULL,
    foo VARCHAR(255) NOT NULL,
    "bar#" INTEGER NOT NULL,
    baz VARCHAR(255) COLLATE "RTRIM" NOT NULL,
    PRIMARY KEY (id)
)"#,
        "baz",
        "RTRIM",
    ),
    (
        r#"CREATE TABLE test_collation (
    id INTEGER NOT NULL,
    foo VARCHAR(255) NOT NULL,
    "bar#" INTEGER NOT NULL,
    baz VARCHAR(255) NOT NULL,
    PRIMARY KEY (id)
)"#,
        "baz",
        "BINARY",
    ),
    (
        r#"CREATE TABLE test_collation (id INTEGER NOT NULL,
    foo VARCHAR(255) NOT NULL,
    "bar/" VARCHAR(255) COLLATE "RTRIM" NOT NULL,
    baz VARCHAR(255) COLLATE "RTRIM" NOT NULL,
    PRIMARY KEY (id)
)"#,
        "bar/",
        "RTRIM",
    ),
    (
        r#"CREATE TABLE test_collation (id INTEGER NOT NULL,
    foo VARCHAR(255) NOT NULL,
    "bar/" VARCHAR(255) NOT NULL,
    baz VARCHAR(255) NOT NULL,
    PRIMARY KEY (id)
)"#,
        "bar/",
        "BINARY",
    ),
    (
        r#"CREATE TABLE test_collation (id INTEGER NOT NULL,
    foo VARCHAR(255) COLLATE "RTRIM" NOT NULL,
    "bar/" INTEGER NOT NULL,
    baz VARCHAR(255) COLLATE "RTRIM" NOT NULL,
    PRIMARY KEY (id)
)"#,
        "baz",
        "RTRIM",
    ),
    (
        r#"CREATE TABLE test_collation (
    id INTEGER NOT NULL,
    foo VARCHAR(255) NOT NULL,
    "bar/" INTEGER NOT NULL,
    baz VARCHAR(255) NOT NULL,
    PRIMARY KEY (id)
)"#,
        "baz",
        "BINARY",
    ),
];

#[tokio::test]
async fn testParseColumnCollation() {
    for (ddl, column_name, expected_collation) in DATA_COLUMN_COLLATION {
        let connection = sqlite();
        drop_table_if_exists(&connection, "test_collation").await;
        exec(&connection, ddl).await;

        let schema_manager = connection.create_schema_manager().await.unwrap();
        let table = schema_manager
            .introspect_table_by_unquoted_name("test_collation", None)
            .await
            .unwrap();

        assert_eq!(
            Some((*expected_collation).to_owned()),
            table.get_column(column_name).unwrap().get_collation(),
            "{ddl}"
        );
    }
}

/// `columnCommentProvider()`.
const COLUMN_COMMENTS: &[(&str, &str, &str, &str)] = &[
    (
        "Single column with no comment",
        r#"CREATE TABLE "test_comment" ("id" TEXT DEFAULT 'a' COLLATE RTRIM)"#,
        "id",
        "",
    ),
    (
        "Single column with type comment",
        "CREATE TABLE \"test_comment\" (\"a\" CLOB DEFAULT NULL COLLATE BINARY --Column a comment\n)",
        "a",
        "Column a comment",
    ),
    (
        "Multiple similar columns with type comment 1",
        "CREATE TABLE \"test_comment\" (\n    a TEXT COLLATE RTRIM,\n    \"b\" TEXT DEFAULT 'a' COLLATE RTRIM,\n    \"bb\" CLOB DEFAULT NULL COLLATE BINARY --Column bb comment\n)",
        "b",
        "",
    ),
    (
        "Multiple similar columns with type comment 2",
        "CREATE TABLE \"test_comment\" (\n    a TEXT COLLATE RTRIM, \"bb\" TEXT DEFAULT 'a' COLLATE RTRIM,\n    \"b\" CLOB DEFAULT NULL COLLATE BINARY --Column b comment\n) ",
        "b",
        "Column b comment",
    ),
    (
        "Multiple similar columns on different lines, with type comment 1",
        "CREATE TABLE \"test_comment\" (\n    a TEXT COLLATE RTRIM,\n    \"b\" CLOB DEFAULT NULL COLLATE BINARY, --Column b comment\n    \"bb\" TEXT DEFAULT 'a' COLLATE RTRIM\n)",
        "bb",
        "",
    ),
    (
        "Multiple similar columns on different lines, with type comment 2",
        "CREATE TABLE \"test_comment\" (\n    a TEXT COLLATE RTRIM,\n    \"bb\" CLOB DEFAULT NULL COLLATE BINARY, --Column bb comment\n    \"b\" TEXT DEFAULT 'a' COLLATE RTRIM\n)",
        "bb",
        "Column bb comment",
    ),
    (
        "Column with numeric but no comment 1",
        "CREATE TABLE \"test_comment\" (\n    \"a\" NUMERIC(10, 0) NOT NULL,\n    \"b\" CLOB NOT NULL, --Column b comment\n    \"c\" CHAR(36) NOT NULL --Column c comment\n)",
        "a",
        "",
    ),
    (
        "Column with numeric but no comment 2",
        "CREATE TABLE \"test_comment\" (\n    \"a\" NUMERIC(10, 0) NOT NULL,\n    \"b\" CLOB NOT NULL, --Column b comment\n    \"c\" CHAR(36) NOT NULL --Column c comment\n)",
        "b",
        "Column b comment",
    ),
    (
        "Column with numeric but no comment 3",
        "CREATE TABLE \"test_comment\" (\n    \"a\" NUMERIC(10, 0) NOT NULL,\n    \"b\" CLOB NOT NULL, --Column b comment\n    \"c\" CHAR(36) NOT NULL --Column c comment\n)",
        "c",
        "Column c comment",
    ),
    (
        "Column \"bar\", select \"bar\" with no comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar\" VARCHAR(255) NOT NULL,\n    baz VARCHAR(255) NOT NULL,\n    PRIMARY KEY(id)\n)",
        "bar",
        "",
    ),
    (
        "Column \"bar\", select \"bar\" with type comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar\" VARCHAR(255) NOT NULL, --Column bar comment\n    baz VARCHAR(255) NOT NULL, --Column baz comment\n    PRIMARY KEY(id)\n)",
        "bar",
        "Column bar comment",
    ),
    (
        "Column \"bar\", select \"baz\" with no comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar\" INTEGER NOT NULL,\n    baz VARCHAR(255) NOT NULL,\n    PRIMARY KEY(id)\n)",
        "baz",
        "",
    ),
    (
        "Column \"bar\", select \"baz\" with type comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar\" INTEGER NOT NULL, --Column bar comment\n    baz VARCHAR(255) NOT NULL, --Column baz comment\n    PRIMARY KEY(id)\n)",
        "baz",
        "Column baz comment",
    ),
    (
        "Column \"bar#\", select \"bar#\" with no comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar#\" VARCHAR(255) NOT NULL,\n    baz VARCHAR(255) NOT NULL,\n    PRIMARY KEY(id)\n)",
        "bar#",
        "",
    ),
    (
        "Column \"bar#\", select \"bar#\" with type comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar#\" VARCHAR(255) NOT NULL, --Column bar comment\n    baz VARCHAR(255) NOT NULL, --Column baz comment\n    PRIMARY KEY(id)\n)",
        "bar#",
        "Column bar comment",
    ),
    (
        "Column \"bar#\", select \"baz\" with no comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar#\" INTEGER NOT NULL,\n    baz VARCHAR(255) NOT NULL,\n    PRIMARY KEY(id)\n)",
        "baz",
        "",
    ),
    (
        "Column \"bar#\", select \"baz\" with type comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar#\" INTEGER NOT NULL, --Column bar comment\n    baz VARCHAR(255) NOT NULL, --Column baz comment\n    PRIMARY KEY(id)\n)",
        "baz",
        "Column baz comment",
    ),
    (
        "Column \"bar/\", select \"bar/\" with no comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar/\" VARCHAR(255) NOT NULL,\n    baz VARCHAR(255) NOT NULL,\n    PRIMARY KEY(id)\n    )",
        "bar/",
        "",
    ),
    (
        "Column \"bar/\", select \"bar/\" with type comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar/\" VARCHAR(255) NOT NULL, --Column bar comment\n    baz VARCHAR(255) NOT NULL, --Column baz comment\n    PRIMARY KEY(id)\n)",
        "bar/",
        "Column bar comment",
    ),
    (
        "Column \"bar/\", select \"baz\" with no comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar/\" INTEGER NOT NULL,\n    baz VARCHAR(255) NOT NULL,\n    PRIMARY KEY(id)\n)",
        "baz",
        "",
    ),
    (
        "Column \"bar/\", select \"baz\" with type comment",
        "CREATE TABLE \"test_comment\" (\n    id INTEGER NOT NULL,\n    foo VARCHAR(255) NOT NULL,\n    \"bar/\" INTEGER NOT NULL, --Column bar comment\n    baz VARCHAR(255) NOT NULL, --Column baz comment\n    PRIMARY KEY(id)\n)",
        "baz",
        "Column baz comment",
    ),
];

#[tokio::test]
async fn testParseColumnCommentFromSQL() {
    for (case, ddl, column_name, expected_comment) in COLUMN_COMMENTS {
        let connection = sqlite();
        drop_table_if_exists(&connection, "test_comment").await;
        exec(&connection, ddl).await;

        let schema_manager = connection.create_schema_manager().await.unwrap();
        let table = schema_manager
            .introspect_table_by_unquoted_name("test_comment", None)
            .await
            .unwrap();

        assert_eq!(
            *expected_comment,
            table.get_column(column_name).unwrap().get_comment(),
            "{case}"
        );
    }
}

/// `getExpectedDefaultSchemaName()` is `null` on `SQLite`.
#[tokio::test]
async fn testDefaultSchemaName() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;

    assert_eq!(None, sm.create_schema_config().await.unwrap().get_name());
}

// The tests SQLiteSchemaManagerTest inherits from
// SchemaManagerFunctionalTestCase.

#[tokio::test]
async fn testListTables() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_test_table(&connection, "list_tables_test").await;
    let tables = sm.list_tables().await.unwrap();

    let table = find_table_by_shortest_name(&tables, "list_tables_test");
    assert!(table.is_some());
    let table = table.unwrap();

    assert!(table.has_column("id"));
    assert!(table.has_column("test"));
    assert!(table.has_column("foreign_key_test"));
}

#[tokio::test]
async fn testListTablesDoesNotIncludeViews() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_test_table(&connection, "test_table_for_view").await;

    let sql = "SELECT * FROM test_table_for_view";

    let view = View::editor()
        .set_unquoted_name("test_view", None)
        .set_sql(sql)
        .create()
        .unwrap();

    sm.create_view(&view).await.unwrap();

    let tables = sm.list_tables().await.unwrap();
    let view = find_table_by_shortest_name(&tables, "test_view");
    assert!(view.is_none());
}

/// The configuration's filter is set before the connection is made (a
/// Rust `Configuration` is the connection's, read-only).
#[tokio::test]
async fn testListTablesWithFilter() {
    // tableFilterProvider()
    for (case, prefix, expected_count) in [
        ("One table", "filter_test_1", 1),
        ("Two tables", "filter_test_", 2),
    ] {
        let mut configuration = Configuration::new();
        let prefix_owned = prefix.to_owned();
        configuration.set_schema_assets_filter(Arc::new(move |name: &str| {
            name.to_lowercase().starts_with(&prefix_owned)
        }));
        let connection = sqlite_with(Some(configuration));
        let sm = schema_manager(&connection).await;

        create_test_table(&connection, "filter_test_1").await;
        create_test_table(&connection, "filter_test_2").await;

        assert_eq!(
            expected_count,
            sm.introspect_table_names().await.unwrap().len(),
            "{case}"
        );
        assert_eq!(
            expected_count,
            sm.introspect_tables().await.unwrap().len(),
            "{case}"
        );
    }
}

#[tokio::test]
async fn testRenameTable() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_test_table(&connection, "old_name").await;
    sm.rename_table("old_name", "new_name").await.unwrap();

    assert!(!sm.tables_exist(&["old_name"]).await.unwrap());
    assert!(sm.tables_exist(&["new_name"]).await.unwrap());
}

/// `createListTableColumns()`, with `SQLiteSchemaManagerTest`'s
/// autoincrement `id`.
fn create_list_table_columns() -> Table {
    let mut table = Table::editor()
        .set_unquoted_name("list_table_columns", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("test", Types::STRING)
                .set_length(Some(255))
                .set_not_null(false)
                .set_default_value(Value::string("expected default"))
                .create()
                .unwrap(),
            column("foo", Types::TEXT).create().unwrap(),
            column("bar", Types::DECIMAL)
                .set_precision(Some(10))
                .set_scale(4)
                .set_not_null(false)
                .create()
                .unwrap(),
            column("baz1", Types::DATETIME_MUTABLE).create().unwrap(),
            column("baz2", Types::TIME_MUTABLE).create().unwrap(),
            column("baz3", Types::DATE_MUTABLE).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();
    table.get_column_mut("id").unwrap().set_autoincrement(true);
    table
}

#[tokio::test]
async fn testListTableColumns() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = create_list_table_columns();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("list_table_columns", None)
        .await
        .unwrap();

    assert_eq!(7, columns.len());

    let [id, test, foo, bar, baz1, baz2, baz3] = &columns[..] else {
        unreachable!()
    };

    let date_types = [
        Types::TIME_MUTABLE,
        Types::DATE_MUTABLE,
        Types::DATETIME_MUTABLE,
    ];

    assert_unqualified_name_equals("id", &id.get_object_name().unwrap());
    assert_eq!(Types::INTEGER, id.get_type_name());
    assert!(!id.get_unsigned());
    assert!(id.get_notnull());
    assert!(id.get_default().is_null());

    assert_unqualified_name_equals("test", &test.get_object_name().unwrap());
    assert_eq!(Types::STRING, test.get_type_name());
    assert_eq!(Some(255), test.get_length());
    assert!(!test.get_fixed());
    assert!(!test.get_notnull());
    assert!(phpunit_equals(
        &Value::string("expected default"),
        test.get_default()
    ));

    assert_unqualified_name_equals("foo", &foo.get_object_name().unwrap());
    assert_eq!(Types::TEXT, foo.get_type_name());
    assert!(!foo.get_unsigned());
    assert!(!foo.get_fixed());
    assert!(foo.get_notnull());
    assert!(foo.get_default().is_null());

    assert_unqualified_name_equals("bar", &bar.get_object_name().unwrap());
    assert_eq!(Types::DECIMAL, bar.get_type_name());
    assert_eq!(None, bar.get_length());
    assert_eq!(Some(10), bar.get_precision());
    assert_eq!(4, bar.get_scale());
    assert!(!bar.get_unsigned());
    assert!(!bar.get_fixed());
    assert!(!bar.get_notnull());
    assert!(bar.get_default().is_null());

    assert_unqualified_name_equals("baz1", &baz1.get_object_name().unwrap());
    assert_eq!(Types::DATETIME_MUTABLE, baz1.get_type_name());
    assert!(baz1.get_notnull());
    assert!(baz1.get_default().is_null());

    assert_unqualified_name_equals("baz2", &baz2.get_object_name().unwrap());
    assert!(date_types.contains(&baz2.get_type_name()));
    assert!(baz2.get_notnull());
    assert!(baz2.get_default().is_null());

    assert_unqualified_name_equals("baz3", &baz3.get_object_name().unwrap());
    assert!(date_types.contains(&baz3.get_type_name()));
    assert!(baz3.get_notnull());
    assert!(baz3.get_default().is_null());
}

#[tokio::test]
async fn testListTableColumnsWithFixedStringColumn() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_list_table_fixed_string", None)
        .set_columns(vec![
            column("column_char", Types::STRING)
                .set_fixed(true)
                .set_length(Some(2))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    sm.create_table(&table).await.unwrap();

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_list_table_fixed_string", None)
        .await
        .unwrap();
    let column = &columns[0];

    assert_eq!(Types::STRING, column.get_type_name());
    assert!(column.get_fixed());
    assert_eq!(Some(2), column.get_length());
}

#[tokio::test]
async fn testDiffListTableColumns() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let offline_table = create_list_table_columns();
    drop_and_create_table(&connection, &offline_table).await;
    let online_table = sm
        .introspect_table_by_unquoted_name("list_table_columns", None)
        .await
        .unwrap();

    assert!(
        sm.create_comparator()
            .compare_tables(&online_table, &offline_table)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn testListTableIndexes() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = get_test_composite_table(&sm, "list_table_indexes_test")
        .await
        .edit()
        .unwrap()
        .add_index(
            Index::editor()
                .set_unquoted_name("test_index_name")
                .set_unquoted_column_names(&["test"])
                .set_type(IndexType::Unique)
                .create()
                .unwrap(),
        )
        .add_index(index("test_composite_idx", &["id", "test"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table_indexes = sm
        .list_table_indexes("list_table_indexes_test")
        .await
        .unwrap();

    assert_eq!(3, table_indexes.len());

    let primary = table_indexes.get("primary");
    assert!(
        primary.is_some(),
        "listTableIndexes() has to return a \"primary\" array key."
    );
    let primary = primary.unwrap();
    assert_eq!(vec!["id", "other_id"], lower(primary.get_columns()));
    assert!(primary.is_unique());
    assert!(primary.is_primary());

    let test_index_name = table_indexes.get("test_index_name").unwrap();
    assert_unqualified_name_equals(
        "test_index_name",
        &test_index_name.get_object_name().unwrap(),
    );
    assert_eq!(vec!["test"], lower(test_index_name.get_columns()));
    assert!(test_index_name.is_unique());
    assert!(!test_index_name.is_primary());

    let test_composite_idx = table_indexes.get("test_composite_idx").unwrap();
    assert_unqualified_name_equals(
        "test_composite_idx",
        &test_composite_idx.get_object_name().unwrap(),
    );
    assert_eq!(vec!["id", "test"], lower(test_composite_idx.get_columns()));
    assert!(!test_composite_idx.is_unique());
    assert!(!test_composite_idx.is_primary());
}

#[tokio::test]
async fn testDropAndCreateIndex() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = get_test_table(&sm, "test_create_index", PhpArray::new())
        .await
        .edit()
        .unwrap()
        .add_index(
            Index::editor()
                .set_unquoted_name("test")
                .set_unquoted_column_names(&["test"])
                .set_type(IndexType::Unique)
                .create()
                .unwrap(),
        )
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let platform = platform(&connection).await;

    let index = table.get_index("test").unwrap();
    let table_sql = table.get_object_name().unwrap().to_sql(platform);
    sm.drop_index(
        &index.get_object_name().unwrap().to_sql(platform),
        &table_sql,
    )
    .await
    .unwrap();

    sm.create_index(index, &table_sql).await.unwrap();
    let table_indexes = sm.list_table_indexes("test_create_index").await.unwrap();

    let test = table_indexes.get("test").unwrap();
    assert_unqualified_name_equals("test", &test.get_object_name().unwrap());
    assert_eq!(vec!["test"], lower(test.get_columns()));
    assert!(test.is_unique());
    assert!(!test.is_primary());
}

#[tokio::test]
async fn testSchemaIntrospection() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_test_table(&connection, "test_table").await;

    let schema = sm.introspect_schema().await.unwrap();
    assert!(schema.has_table("test_table"));
}

/// `SchemaEditor::modifyTableByUnquotedName()` is not ported: the table is
/// edited and put back in place of the old one.
#[tokio::test]
async fn testMigrateSchema() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_test_table(&connection, "table_to_alter").await;
    create_test_table(&connection, "table_to_drop").await;

    let schema = sm.introspect_schema().await.unwrap();
    let schema_config = sm.create_schema_config().await.unwrap();

    let table_to_create = Table::editor()
        .set_configuration(schema_config.to_table_configuration())
        .set_unquoted_name("table_to_create", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_not_null(true)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_options(schema_config.get_default_table_options().clone())
        .create()
        .unwrap();

    let table_to_alter = schema
        .get_table("table_to_alter")
        .unwrap()
        .edit()
        .unwrap()
        .drop_column_by_unquoted_name("foreign_key_test")
        .add_column(column("number", Types::INTEGER).create().unwrap())
        .create()
        .unwrap();

    let schema = schema
        .edit()
        .drop_table_by_unquoted_name("table_to_alter", None)
        .add_table(table_to_alter)
        .drop_table_by_unquoted_name("table_to_drop", None)
        .add_table(table_to_create)
        .create()
        .unwrap();

    sm.migrate_schema(&schema).await.unwrap();

    let schema = sm.introspect_schema().await.unwrap();

    assert!(schema.has_table("table_to_alter"));
    assert!(
        !schema
            .get_table("table_to_alter")
            .unwrap()
            .has_column("foreign_key_test")
    );
    assert!(
        schema
            .get_table("table_to_alter")
            .unwrap()
            .has_column("number")
    );
    assert!(!schema.has_table("table_to_drop"));
    assert!(schema.has_table("table_to_create"));
}

#[tokio::test]
async fn testAlterTableScenario() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_test_table(&connection, "alter_table").await;
    create_test_table(&connection, "alter_table_foreign").await;

    let table = sm.introspect_table("alter_table").await.unwrap();
    assert!(table.has_column("id"));
    assert!(table.has_column("test"));
    assert!(table.has_column("foreign_key_test"));
    assert_eq!(0, table.get_foreign_keys().len());
    assert_eq!(1, table.get_indexes().len());

    let new_table = table
        .edit()
        .unwrap()
        .add_column(column("foo", Types::INTEGER).create().unwrap())
        .drop_column_by_unquoted_name("test")
        .create()
        .unwrap();

    let comparator = sm.create_comparator();

    let diff = comparator.compare_tables(&table, &new_table).unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm.introspect_table("alter_table").await.unwrap();
    assert!(!table.has_column("test"));
    assert!(table.has_column("foo"));

    let new_table = table
        .edit()
        .unwrap()
        .add_index(index("foo_idx", &["foo"]))
        .create()
        .unwrap();

    let diff = comparator.compare_tables(&table, &new_table).unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm.introspect_table("alter_table").await.unwrap();
    assert_eq!(2, table.get_indexes().len());
    assert!(table.has_index("foo_idx"));
    assert_eq!(
        vec!["foo"],
        lower(table.get_index("foo_idx").unwrap().get_columns())
    );
    assert!(!table.get_index("foo_idx").unwrap().is_primary());
    assert!(!table.get_index("foo_idx").unwrap().is_unique());

    let foo_index = index("foo_idx", &["foo", "foreign_key_test"]);

    let new_table = table
        .edit()
        .unwrap()
        .drop_index_by_unquoted_name("foo_idx")
        .add_index(foo_index.clone())
        .create()
        .unwrap();

    let diff = comparator.compare_tables(&table, &new_table).unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm.introspect_table("alter_table").await.unwrap();
    assert_eq!(2, table.get_indexes().len());
    assert!(table.has_index("foo_idx"));

    assert_index_equals(&foo_index, table.get_index("foo_idx").unwrap());

    let bar_index = index("bar_idx", &["foo", "foreign_key_test"]);

    let new_table = table
        .edit()
        .unwrap()
        .drop_index_by_unquoted_name("foo_idx")
        .add_index(bar_index.clone())
        .create()
        .unwrap();

    let diff = comparator.compare_tables(&table, &new_table).unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm.introspect_table("alter_table").await.unwrap();
    assert_eq!(2, table.get_indexes().len());
    assert!(table.has_index("bar_idx"));
    assert!(!table.has_index("foo_idx"));

    assert_index_equals(&bar_index, table.get_index("bar_idx").unwrap());

    let new_table = table
        .edit()
        .unwrap()
        .drop_index_by_unquoted_name("bar_idx")
        .add_foreign_key_constraint(foreign_key(
            None,
            &["foreign_key_test"],
            "alter_table_foreign",
            &["id"],
        ))
        .create()
        .unwrap();

    let diff = comparator.compare_tables(&table, &new_table).unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm.introspect_table("alter_table").await.unwrap();

    // don't check for index size here, some platforms automatically add
    // indexes for foreign keys.
    assert!(!table.has_index("bar_idx"));

    let fks: Vec<&ForeignKeyConstraint> = table.get_foreign_keys().values().collect();
    assert_eq!(1, fks.len());
    let foreign_key = fks[0];

    assert_eq!(
        "alter_table_foreign",
        foreign_key.get_foreign_table_name().to_lowercase()
    );
    assert_eq!(
        vec!["foreign_key_test"],
        lower(foreign_key.get_local_columns())
    );
    assert_eq!(vec!["id"], lower(foreign_key.get_foreign_columns()));
}

#[tokio::test]
async fn testAlterIntrospectedTablePreservesIndexesAndForeignKeys() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let referenced_table = Table::editor()
        .set_unquoted_name("alter_introspected_ref", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let indexes = vec![
        Index::editor()
            .set_unquoted_name("idx_intro_name")
            .set_unquoted_column_names(&["name"])
            .set_type(IndexType::Unique)
            .create()
            .unwrap(),
        index("idx_intro_ref", &["ref_id"]),
    ];

    let foreign_key_constraints = vec![foreign_key(
        Some("fk_intro_ref"),
        &["ref_id"],
        "alter_introspected_ref",
        &["id"],
    )];

    let table = Table::editor()
        .set_unquoted_name("alter_introspected", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("ref_id", Types::INTEGER).create().unwrap(),
            column("name", Types::STRING)
                .set_length(Some(32))
                .create()
                .unwrap(),
            column("weight", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_indexes(indexes.clone())
        .set_foreign_key_constraints(foreign_key_constraints)
        .create()
        .unwrap();

    let platform = platform(&connection).await;

    drop_table_if_exists(
        &connection,
        &table.get_object_name().unwrap().to_sql(platform),
    )
    .await;
    drop_table_if_exists(
        &connection,
        &referenced_table.get_object_name().unwrap().to_sql(platform),
    )
    .await;

    sm.create_table(&referenced_table).await.unwrap();
    sm.create_table(&table).await.unwrap();

    let old_table = sm
        .introspect_table_by_unquoted_name("alter_introspected", None)
        .await
        .unwrap();

    // Guard against a vacuous comparison of the foreign keys below.
    assert_eq!(1, old_table.get_foreign_keys().len());

    // Modify an unrelated column to force a table alteration that does not
    // touch the indexed or referencing columns.
    let new_table = old_table
        .edit()
        .unwrap()
        .modify_column_by_unquoted_name("weight", |editor| {
            editor.set_type_name(Types::STRING).set_length(Some(32))
        })
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .compare_tables(&old_table, &new_table)
        .unwrap();
    sm.alter_table(&diff).await.unwrap();

    let table = sm
        .introspect_table_by_unquoted_name("alter_introspected", None)
        .await
        .unwrap();

    // The indexes and foreign keys must survive the alteration.
    assert_index_equals(&indexes[0], table.get_index("idx_intro_name").unwrap());
    assert_index_equals(&indexes[1], table.get_index("idx_intro_ref").unwrap());
    assert_foreign_key_constraint_list_equals(
        &old_table.get_foreign_keys().values().collect::<Vec<_>>(),
        &table.get_foreign_keys().values().collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn testCreateAndListViews() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_test_table(&connection, "view_test_table").await;

    let name = "doctrine_test_view";
    let sql = "SELECT * FROM view_test_table";

    let view = View::editor()
        .set_unquoted_name(name, None)
        .set_sql(sql)
        .create()
        .unwrap();

    sm.create_view(&view).await.unwrap();

    let views = sm.list_views().await.unwrap();

    let found = views
        .iter()
        .find(|v| v.get_shortest_name(v.get_namespace_name()) == name);
    assert!(found.is_some());

    assert!(found.unwrap().get_sql().contains("view_test_table"));
}

#[tokio::test]
async fn testUpdateSchemaWithForeignKeyRenaming() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let configuration = sm
        .create_schema_config()
        .await
        .unwrap()
        .to_table_configuration();
    let table = Table::editor()
        .set_unquoted_name("test_fk_base", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let table_fk = Table::editor()
        .set_unquoted_name("test_fk_rename", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("fk_id", Types::INTEGER).create().unwrap(),
        ])
        .set_indexes(vec![index("fk_idx", &["fk_id"])])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_foreign_key_constraints(vec![foreign_key(None, &["fk_id"], "test_fk_base", &["id"])])
        .set_configuration(configuration)
        .create()
        .unwrap();

    let platform = platform(&connection).await;

    drop_table_if_exists(
        &connection,
        &table_fk.get_object_name().unwrap().to_sql(platform),
    )
    .await;
    drop_table_if_exists(
        &connection,
        &table.get_object_name().unwrap().to_sql(platform),
    )
    .await;

    sm.create_table(&table).await.unwrap();
    sm.create_table(&table_fk).await.unwrap();

    let table_fk_new = Table::editor()
        .set_unquoted_name("test_fk_rename", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("rename_fk_id", Types::INTEGER).create().unwrap(),
        ])
        .set_indexes(vec![index("fk_idx", &["rename_fk_id"])])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_foreign_key_constraints(vec![foreign_key(
            None,
            &["rename_fk_id"],
            "test_fk_base",
            &["id"],
        )])
        .set_configuration(configuration)
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .compare_tables(
            &sm.introspect_table(&table_fk.get_object_name().unwrap().to_string())
                .await
                .unwrap(),
            &table_fk_new,
        )
        .unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm.introspect_table("test_fk_rename").await.unwrap();
    assert!(table.has_column("rename_fk_id"));

    let foreign_keys: Vec<&ForeignKeyConstraint> = table.get_foreign_keys().values().collect();
    assert_eq!(1, foreign_keys.len());
    let foreign_key = foreign_keys[0];

    assert_eq!(vec!["rename_fk_id"], lower(foreign_key.get_local_columns()));
}

#[tokio::test]
async fn testRenameIndexUsedInForeignKeyConstraint() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let primary_table = Table::editor()
        .set_unquoted_name("test_rename_index_primary", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let foreign_table = Table::editor()
        .set_unquoted_name("test_rename_index_foreign", None)
        .set_columns(vec![column("fk", Types::INTEGER).create().unwrap()])
        .set_indexes(vec![index("rename_index_fk_idx", &["fk"])])
        .set_foreign_key_constraints(vec![foreign_key(
            Some("fk_constraint"),
            &["fk"],
            "test_rename_index_primary",
            &["id"],
        )])
        .create()
        .unwrap();

    let platform = platform(&connection).await;

    drop_table_if_exists(
        &connection,
        &foreign_table.get_object_name().unwrap().to_sql(platform),
    )
    .await;
    drop_table_if_exists(
        &connection,
        &primary_table.get_object_name().unwrap().to_sql(platform),
    )
    .await;

    sm.create_table(&primary_table).await.unwrap();
    sm.create_table(&foreign_table).await.unwrap();

    let foreign_table2 = foreign_table
        .edit()
        .unwrap()
        .rename_index_by_unquoted_name("rename_index_fk_idx", "renamed_index_fk_idx")
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .compare_tables(&foreign_table, &foreign_table2)
        .unwrap();

    sm.alter_table(&diff).await.unwrap();

    let foreign_table = sm
        .introspect_table_by_unquoted_name("test_rename_index_foreign", None)
        .await
        .unwrap();

    assert!(!foreign_table.has_index("rename_index_fk_idx"));
    assert!(foreign_table.has_index("renamed_index_fk_idx"));
    assert!(foreign_table.has_foreign_key("fk_constraint"));
}

#[tokio::test]
async fn testChangeColumnsTypeWithDefaultValue() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let old_table = Table::editor()
        .set_unquoted_name("column_def_change_type", None)
        .set_columns(vec![
            column("col_int", "smallint")
                .set_default_value(Value::Int(666))
                .create()
                .unwrap(),
            column("col_string", "string")
                .set_length(Some(3))
                .set_default_value(Value::string("foo"))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &old_table).await;

    let new_table = old_table
        .edit()
        .unwrap()
        .modify_column_by_unquoted_name("col_int", |editor| editor.set_type_name(Types::INTEGER))
        .modify_column_by_unquoted_name("col_string", |editor| editor.set_fixed(true))
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .compare_tables(
            &sm.introspect_table_by_unquoted_name("column_def_change_type", None)
                .await
                .unwrap(),
            &new_table,
        )
        .unwrap();

    sm.alter_table(&diff).await.unwrap();

    let columns = sm
        .introspect_table_columns_by_unquoted_name("column_def_change_type", None)
        .await
        .unwrap();

    assert_eq!(Types::INTEGER, columns[0].get_type_name());
    assert!(phpunit_equals(&Value::Int(666), columns[0].get_default()));

    assert_eq!(Types::STRING, columns[1].get_type_name());
    assert!(phpunit_equals(
        &Value::string("foo"),
        columns[1].get_default()
    ));
}

#[tokio::test]
async fn testListTableWithBlob() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_blob_table", None)
        .set_columns(vec![column("binarydata", Types::BLOB).create().unwrap()])
        .create()
        .unwrap();

    sm.create_table(&table).await.unwrap();

    let created = sm
        .introspect_table_by_unquoted_name("test_blob_table", None)
        .await
        .unwrap();

    assert!(created.has_column("binarydata"));
    assert_eq!(
        Types::BLOB,
        created.get_column("binarydata").unwrap().get_type_name()
    );
}

#[tokio::test]
async fn testListTableFloatTypeColumns() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_float_columns", None)
        .set_columns(vec![
            column("col_float", Types::FLOAT).create().unwrap(),
            column("col_smallfloat", Types::SMALLFLOAT)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_float_columns", None)
        .await
        .unwrap();

    assert_eq!(Types::FLOAT, columns[0].get_type_name());
    assert_eq!(Types::SMALLFLOAT, columns[1].get_type_name());
    assert!(!columns[0].get_unsigned());
    assert!(!columns[1].get_unsigned());
}

#[tokio::test]
async fn testColumnDefaultLifecycle() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let old_table = Table::editor()
        .set_unquoted_name("col_def_lifecycle", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
            column("column1", Types::STRING)
                .set_length(Some(1))
                .create()
                .unwrap(),
            column("column2", Types::STRING)
                .set_length(Some(1))
                .set_default_value(Value::string(""))
                .create()
                .unwrap(),
            column("column3", Types::STRING)
                .set_length(Some(8))
                .set_default_value(Value::string("default1"))
                .create()
                .unwrap(),
            column("column4", Types::INTEGER)
                .set_default_value(Value::Int(0))
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &old_table).await;

    let old_table = sm
        .introspect_table_by_unquoted_name("col_def_lifecycle", None)
        .await
        .unwrap();

    let default = |table: &Table, name: &str| table.get_column(name).unwrap().get_default().clone();
    assert!(default(&old_table, "id").is_null());
    assert!(default(&old_table, "column1").is_null());
    assert!(Value::string("").identical(&default(&old_table, "column2")));
    assert!(Value::string("default1").identical(&default(&old_table, "column3")));
    assert!(Value::string("0").identical(&default(&old_table, "column4")));

    let new_table = old_table
        .edit()
        .unwrap()
        .modify_column_by_unquoted_name("column1", |editor| {
            editor.set_default_value(Value::string(""))
        })
        .modify_column_by_unquoted_name("column2", |editor| editor.set_default_value(Value::Null))
        .modify_column_by_unquoted_name("column3", |editor| {
            editor.set_default_value(Value::string("default2"))
        })
        .modify_column_by_unquoted_name("column4", |editor| editor.set_default_value(Value::Null))
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .compare_tables(
            &sm.introspect_table_by_unquoted_name("col_def_lifecycle", None)
                .await
                .unwrap(),
            &new_table,
        )
        .unwrap();

    sm.alter_table(&diff).await.unwrap();

    let columns = sm
        .introspect_table_columns_by_unquoted_name("col_def_lifecycle", None)
        .await
        .unwrap();

    assert!(Value::string("").identical(columns[1].get_default()));
    assert!(columns[2].get_default().is_null());
    assert!(Value::string("default2").identical(columns[3].get_default()));
    assert!(columns[4].get_default().is_null());
}

/// `SQLite` stores BINARY and VARBINARY columns as BLOB (the test's
/// `assertBinaryColumnIsValid()` and `assertVarBinaryColumnIsValid()`
/// overrides).
#[tokio::test]
async fn testListTableWithBinary() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_binary_table", None)
        .set_columns(vec![
            column("column_binary", Types::BINARY)
                .set_length(Some(16))
                .set_fixed(true)
                .create()
                .unwrap(),
            column("column_varbinary", Types::BINARY)
                .set_length(Some(32))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    sm.create_table(&table).await.unwrap();

    let table = sm
        .introspect_table_by_unquoted_name("test_binary_table", None)
        .await
        .unwrap();
    assert_eq!(
        Types::BLOB,
        table.get_column("column_binary").unwrap().get_type_name()
    );
    assert_eq!(
        Types::BLOB,
        table
            .get_column("column_varbinary")
            .unwrap()
            .get_type_name()
    );
}

#[tokio::test]
async fn testGetNonExistingTable() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;

    let e = sm
        .introspect_table_by_unquoted_name("non_existing", None)
        .await
        .unwrap_err();
    assert!(
        e.is_instance_of("Doctrine\\DBAL\\Schema\\SchemaException"),
        "{e}"
    );
}

#[tokio::test]
async fn testDoesNotListIndexesImplicitlyCreatedByForeignKeys() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let primary_table = Table::editor()
        .set_unquoted_name("test_list_index_impl_primary", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let foreign_table = Table::editor()
        .set_unquoted_name("test_list_index_impl_foreign", None)
        .set_columns(vec![
            column("fk1", Types::INTEGER).create().unwrap(),
            column("fk2", Types::INTEGER).create().unwrap(),
        ])
        .set_indexes(vec![index("explicit_fk1_idx", &["fk1"])])
        .set_foreign_key_constraints(vec![
            foreign_key(None, &["fk1"], "test_list_index_impl_primary", &["id"]),
            foreign_key(None, &["fk2"], "test_list_index_impl_primary", &["id"]),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &primary_table).await;
    drop_and_create_table(&connection, &foreign_table).await;

    assert_index_list_equals(
        &[
            Index::editor()
                .set_name(Some(
                    UnqualifiedName::unquoted("IDX_3D6C147FDC58D6C").unwrap(),
                ))
                .set_column_names(vec![UnqualifiedName::unquoted("fk2").unwrap()])
                .create()
                .unwrap(),
            Index::editor()
                .set_name(Some(UnqualifiedName::unquoted("explicit_fk1_idx").unwrap()))
                .set_column_names(vec![UnqualifiedName::unquoted("fk1").unwrap()])
                .create()
                .unwrap(),
        ],
        &sm.introspect_table_indexes_by_unquoted_name("test_list_index_impl_foreign", None)
            .await
            .unwrap(),
    );
}

#[tokio::test]
async fn testCommentInTable() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("table_with_comment", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_comment("'\\ Foo with control characters '\\")
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table = sm
        .introspect_table_by_unquoted_name("table_with_comment", None)
        .await
        .unwrap();
    assert_eq!(
        Some("'\\ Foo with control characters '\\".to_owned()),
        table.get_comment()
    );
}

/// `createReservedKeywordTables()`.
async fn create_reserved_keyword_tables(connection: &Connection) {
    let platform = platform(connection).await;
    drop_table_if_exists(connection, &platform.quote_single_identifier("user")).await;
    drop_table_if_exists(connection, &platform.quote_single_identifier("group")).await;

    let user = Table::editor()
        .set_unquoted_name("user", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("group_id", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_foreign_key_constraints(vec![foreign_key(None, &["group_id"], "group", &["id"])])
        .create()
        .unwrap();

    let group = Table::editor()
        .set_unquoted_name("group", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let schema = Schema::editor()
        .set_tables(vec![user, group])
        .create()
        .unwrap();

    let schema_manager = connection.create_schema_manager().await.unwrap();
    schema_manager.create_schema_objects(&schema).await.unwrap();
}

#[tokio::test]
async fn testIntrospectReservedKeywordTableViaListTableDetails() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_reserved_keyword_tables(&connection).await;

    let user = sm
        .introspect_table_by_quoted_name("user", None)
        .await
        .unwrap();
    assert_eq!(2, user.get_columns().len());
    assert_eq!(2, user.get_indexes().len());
    assert_eq!(1, user.get_foreign_keys().len());
}

#[tokio::test]
async fn testIntrospectReservedKeywordTableViaListTables() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    create_reserved_keyword_tables(&connection).await;

    let tables = sm.list_tables().await.unwrap();

    let user = find_table_by_shortest_name(&tables, "user");
    assert!(user.is_some());
    let user = user.unwrap();
    assert_eq!(2, user.get_columns().len());
    assert_eq!(2, user.get_indexes().len());
    assert_eq!(1, user.get_foreign_keys().len());
}

#[tokio::test]
async fn testChangeIndexWithForeignKeys() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "child").await;
    drop_table_if_exists(&connection, "parent").await;

    let parent = Table::editor()
        .set_unquoted_name("parent", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let child = Table::editor()
        .set_unquoted_name("child", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("parent_id", Types::INTEGER).create().unwrap(),
        ])
        .set_indexes(vec![index("idx_1", &["parent_id"])])
        .set_foreign_key_constraints(vec![foreign_key(None, &["parent_id"], "parent", &["id"])])
        .create()
        .unwrap();

    let schema = Schema::editor()
        .set_tables(vec![parent, child.clone()])
        .create()
        .unwrap();

    let schema_manager = connection.create_schema_manager().await.unwrap();
    schema_manager.create_schema_objects(&schema).await.unwrap();

    let child = child
        .edit()
        .unwrap()
        .drop_index_by_unquoted_name("idx_1")
        .add_index(index("idx_2", &["parent_id"]))
        .create()
        .unwrap();

    let diff = schema_manager
        .create_comparator_with(ComparatorConfig::default().with_detect_renamed_indexes(false))
        .compare_tables(
            &schema_manager.introspect_table("child").await.unwrap(),
            &child,
        )
        .unwrap();

    schema_manager.alter_table(&diff).await.unwrap();

    let child = schema_manager
        .introspect_table_by_unquoted_name("child", None)
        .await
        .unwrap();
    assert!(!child.has_index("idx_1"));
    assert!(child.has_index("idx_2"));
}

#[tokio::test]
async fn testSwitchPrimaryKeyOrder() {
    let connection = sqlite();
    let prototype = Table::editor()
        .set_unquoted_name("test_switch_pk_order", None)
        .set_columns(vec![
            column("foo_id", Types::INTEGER).create().unwrap(),
            column("bar_id", Types::INTEGER).create().unwrap(),
        ])
        .create()
        .unwrap();

    let mut table = prototype.clone();
    table.set_primary_key(&["foo_id", "bar_id"], None).unwrap();
    drop_and_create_table(&connection, &table).await;

    let mut table = prototype.clone();
    table.set_primary_key(&["bar_id", "foo_id"], None).unwrap();

    let schema_manager = connection.create_schema_manager().await.unwrap();
    let diff = schema_manager
        .create_comparator()
        .compare_tables(
            &schema_manager
                .introspect_table("test_switch_pk_order")
                .await
                .unwrap(),
            &table,
        )
        .unwrap();
    assert!(!diff.is_empty());

    schema_manager.alter_table(&diff).await.unwrap();

    let table = schema_manager
        .introspect_table("test_switch_pk_order")
        .await
        .unwrap();
    let primary_key = table.get_primary_key();
    assert!(primary_key.is_some());
    assert_eq!(
        vec!["bar_id", "foo_id"],
        lower(primary_key.unwrap().get_columns())
    );
}

#[tokio::test]
async fn testDropColumnWithDefault() {
    let connection = sqlite();
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("drop_column_with_default", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("todrop", Types::INTEGER)
                .set_default_value(Value::Int(10))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table = table
        .edit()
        .unwrap()
        .drop_column_by_unquoted_name("todrop")
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .compare_tables(
            &sm.introspect_table_by_unquoted_name("drop_column_with_default", None)
                .await
                .unwrap(),
            &table,
        )
        .unwrap();

    sm.alter_table(&diff).await.unwrap();

    let columns = sm
        .introspect_table_columns_by_unquoted_name("drop_column_with_default", None)
        .await
        .unwrap();
    assert_eq!(1, columns.len());
}
