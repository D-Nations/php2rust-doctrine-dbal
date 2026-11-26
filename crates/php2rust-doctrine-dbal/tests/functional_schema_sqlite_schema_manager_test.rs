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

mod schema_manager_support;

use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::ParameterTypes;
use php2rust_doctrine_dbal::deprecations;
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, ColumnDiff, ForeignKeyConstraint, Table, TableDiff, TableDiffParts,
};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{
    assert_foreign_key_equals, assert_unqualified_name_equals, column, drop_and_create_table,
    drop_table_if_exists, exec, platform, primary_key, schema_manager, sqlite,
};

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

/// SQLiteSchemaManagerTest's own `testPrimaryKeyAutoIncrement()`, which
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
