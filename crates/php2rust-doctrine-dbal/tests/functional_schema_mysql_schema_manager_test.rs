//! Port of the tests tests/Functional/Schema/MySQLSchemaManagerTest.php
//! defines, with tests/Functional/Schema/MySQL/PointType.php and
//! CustomType.php, on a live `MySQL` server: the `pdo_mysql` cargo
//! feature and `DBAL_TEST_MYSQL_URL` (without the variable each test
//! passes with a note). The tests it inherits from
//! `SchemaManagerFunctionalTestCase` are not here yet. Names and
//! assertions are PHPUnit's; a test upstream skips on the server's
//! platform returns early.
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/MySQLSchemaManagerTest.php
//! Upstream: doctrine/dbal tests/Functional/Schema/MySQL/PointType.php
//! Upstream: doctrine/dbal tests/Functional/Schema/MySQL/CustomType.php
#![cfg(feature = "pdo_mysql")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod mysql_support;
mod schema_manager_support;

use std::sync::Arc;

use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::platforms::{
    AbstractPlatform, LENGTH_LIMIT_BLOB, LENGTH_LIMIT_MEDIUMBLOB, LENGTH_LIMIT_MEDIUMTEXT,
    LENGTH_LIMIT_TEXT, LENGTH_LIMIT_TINYBLOB, LENGTH_LIMIT_TINYTEXT,
};
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, Column, CurrentTimestamp, DefaultExpression, Index, IndexType,
    PrimaryKeyConstraint, Table,
};
use php2rust_doctrine_dbal::types::{self, Type, TypeArgument, Types};
use php2rust_doctrine_dbal::{Connection, DriverManager, Exception, ParameterTypes, deprecations};
use schema_manager_support::{
    assert_index_list_equals, column, drop_and_create_table, platform, schema_manager,
};

/// `PointType`.
#[derive(Debug)]
struct PointType;

impl Type for PointType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Tests\\Functional\\Schema\\MySQL\\PointType"
    }

    fn get_sql_declaration(
        &self,
        _column: &PhpArray,
        _platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("POINT"))
    }

    fn get_mapped_database_types(&self, _platform: &dyn AbstractPlatform) -> Vec<PhpString> {
        vec![PhpString::from("point")]
    }
}

/// `CustomType extends IntegerType`.
#[derive(Debug)]
struct CustomType;

impl Type for CustomType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Tests\\Functional\\Schema\\MySQL\\CustomType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        types::get_type(Types::INTEGER)
            .unwrap()
            .get_sql_declaration(column, platform)
    }
}

/// `setUpBeforeClass()`.
fn set_up_before_class() {
    if !types::has_type("point") {
        types::add_type("point", TypeArgument::Instance(Arc::new(PointType))).unwrap();
    }
}

/// The test's connection, after `setUpBeforeClass()`.
async fn connection(test: &str) -> Option<mysql_support::Live> {
    set_up_before_class();
    mysql_support::mysql(test).await
}

/// `$this->connection->executeStatement($sql)`.
async fn exec(connection: &Connection, sql: &str) {
    connection
        .execute_statement(sql, &PhpArray::new(), &ParameterTypes::new())
        .await
        .unwrap();
}

/// The name of a column's type (`assertInstanceOf(XType::class, ...)`).
fn type_name(column: &Column) -> String {
    types::lookup_name(&column.get_type().unwrap()).unwrap()
}

fn options(pairs: &[(&str, &str)]) -> PhpArray {
    let mut options = PhpArray::new();
    for (key, value) in pairs {
        options.insert(*key, Value::string(*value));
    }
    options
}

fn is_mariadb(platform: &(dyn AbstractPlatform + Send + Sync)) -> bool {
    platform.class().contains("MariaDB")
}

fn assert_same_string(expected: &str, actual: &Value) {
    assert!(
        Value::string(expected).identical(actual),
        "{expected} {actual:?}"
    );
}

#[tokio::test]
async fn testFulltextIndex() {
    let Some(connection) = connection("testFulltextIndex").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let index = Index::editor()
        .set_unquoted_name("f_index")
        .set_type(IndexType::Fulltext)
        .set_unquoted_column_names(&["text"])
        .create()
        .unwrap();

    let mut table = Table::editor()
        .set_unquoted_name("fulltext_index", None)
        .set_columns(vec![column("text", Types::TEXT).create().unwrap()])
        .set_indexes(vec![index])
        .set_options(options(&[("engine", "MyISAM")]))
        .create()
        .unwrap();

    table.get_index_mut("f_index").unwrap().add_flag("fulltext");
    let index = table.get_index("f_index").unwrap().clone();

    drop_and_create_table(&connection, &table).await;

    assert_index_list_equals(
        &[index],
        &sm.introspect_table_indexes_by_unquoted_name("fulltext_index", None)
            .await
            .unwrap(),
    );
}

#[tokio::test]
async fn testSpatialIndex() {
    let Some(connection) = connection("testSpatialIndex").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let index = Index::editor()
        .set_unquoted_name("s_index")
        .set_type(IndexType::Spatial)
        .set_unquoted_column_names(&["point"])
        .create()
        .unwrap();

    let mut table = Table::editor()
        .set_unquoted_name("spatial_index", None)
        .set_columns(vec![column("point", "point").create().unwrap()])
        .set_indexes(vec![index])
        .set_options(options(&[("engine", "MyISAM")]))
        .create()
        .unwrap();

    table.get_index_mut("s_index").unwrap().add_flag("spatial");
    let index = table.get_index("s_index").unwrap().clone();

    drop_and_create_table(&connection, &table).await;

    assert_index_list_equals(
        &[index],
        &sm.introspect_table_indexes_by_unquoted_name("spatial_index", None)
            .await
            .unwrap(),
    );
}

#[tokio::test]
async fn testIndexWithLength() {
    let Some(connection) = connection("testIndexWithLength").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let index = Index::editor()
        .set_unquoted_name("text_index")
        .add_unquoted_column_name("text", Some(128))
        .create()
        .unwrap();

    let table = Table::editor()
        .set_unquoted_name("index_length", None)
        .set_columns(vec![
            column("text", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
        ])
        .set_indexes(vec![index.clone()])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    assert_index_list_equals(
        &[index],
        &sm.introspect_table_indexes_by_unquoted_name("index_length", None)
            .await
            .unwrap(),
    );
}

#[tokio::test]
async fn testDropPrimaryKeyWithAutoincrementColumn() {
    let Some(connection) = connection("testDropPrimaryKeyWithAutoincrementColumn").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("drop_primary_key", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
            column("foo", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(Some(
            PrimaryKeyConstraint::editor()
                .set_unquoted_column_names(&["id", "foo"])
                .create()
                .unwrap(),
        ))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let mut diff_table = table
        .edit()
        .unwrap()
        .drop_primary_key_constraint()
        .create()
        .unwrap();

    deprecations::enable_tracking_deprecations();
    let before = deprecations::count("https://github.com/doctrine/dbal/pull/6841");
    diff_table.drop_primary_key().unwrap();

    let diff = sm
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(&table, &diff_table)
        .unwrap();

    sm.alter_table(&diff).await.unwrap();
    assert!(deprecations::count("https://github.com/doctrine/dbal/pull/6841") > before);

    let table = sm
        .introspect_table_by_unquoted_name("drop_primary_key", None)
        .await
        .unwrap();

    assert!(table.get_primary_key().is_none());
    assert!(!table.get_column("id").unwrap().get_autoincrement());
}

#[tokio::test]
async fn testDoesNotPropagateDefaultValuesForUnsupportedColumnTypes() {
    let Some(connection) =
        connection("testDoesNotPropagateDefaultValuesForUnsupportedColumnTypes").await
    else {
        return;
    };
    if is_mariadb(platform(&connection).await) {
        // MariaDB supports default values for BLOB and TEXT columns and
        // will propagate values.
        return;
    }
    let sm = schema_manager(&connection).await;
    let def = || Value::string("def");
    let table = Table::editor()
        .set_unquoted_name("text_blob_default_value", None)
        .set_columns(vec![
            column("def_text", Types::TEXT)
                .set_default_value(def())
                .create()
                .unwrap(),
            column("def_text_null", Types::TEXT)
                .set_not_null(false)
                .set_default_value(def())
                .create()
                .unwrap(),
            column("def_blob", Types::BLOB)
                .set_default_value(def())
                .create()
                .unwrap(),
            column("def_blob_null", Types::BLOB)
                .set_not_null(false)
                .set_default_value(def())
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let online_table = sm
        .introspect_table_by_unquoted_name("text_blob_default_value", None)
        .await
        .unwrap();

    assert!(
        online_table
            .get_column("def_text")
            .unwrap()
            .get_default()
            .is_null()
    );
    assert!(
        online_table
            .get_column("def_text_null")
            .unwrap()
            .get_default()
            .is_null()
    );
    assert!(
        !online_table
            .get_column("def_text_null")
            .unwrap()
            .get_notnull()
    );
    assert!(
        online_table
            .get_column("def_blob")
            .unwrap()
            .get_default()
            .is_null()
    );
    assert!(
        online_table
            .get_column("def_blob_null")
            .unwrap()
            .get_default()
            .is_null()
    );
    assert!(
        !online_table
            .get_column("def_blob_null")
            .unwrap()
            .get_notnull()
    );

    assert!(
        sm.create_comparator()
            .await
            .unwrap()
            .compare_tables(&online_table, &table)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn testColumnCharset() {
    let Some(connection) = connection("testColumnCharset").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_column_charset", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("foo", Types::TEXT)
                .set_charset(Some("ascii"))
                .create()
                .unwrap(),
            column("bar", Types::TEXT)
                .set_charset(Some("latin1"))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_column_charset", None)
        .await
        .unwrap();

    assert_eq!(None, columns[0].get_charset());
    assert_eq!(Some("ascii".to_owned()), columns[1].get_charset());
    assert_eq!(Some("latin1".to_owned()), columns[2].get_charset());
}

/// `testAlterColumnCharset()` and `testColumnCharsetChange()`: a column's
/// charset changed to `ascii`.
async fn assert_charset_changes_to_ascii(
    connection: &Connection,
    table_name: &str,
    column: Column,
) {
    let sm = schema_manager(connection).await;
    let column_name = column.get_name();
    let table = Table::editor()
        .set_unquoted_name(table_name, None)
        .set_columns(vec![column])
        .create()
        .unwrap();

    drop_and_create_table(connection, &table).await;

    let diff_table = table
        .edit()
        .unwrap()
        .modify_column_by_unquoted_name(&column_name, |editor| editor.set_charset(Some("ascii")))
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(&table, &diff_table)
        .unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm
        .introspect_table_by_unquoted_name(table_name, None)
        .await
        .unwrap();

    assert_eq!(
        Some("ascii".to_owned()),
        table.get_column(&column_name).unwrap().get_charset()
    );
}

#[tokio::test]
async fn testAlterColumnCharset() {
    let Some(connection) = connection("testAlterColumnCharset").await else {
        return;
    };
    assert_charset_changes_to_ascii(
        &connection,
        "test_alter_column_charset",
        column("col_text", Types::TEXT)
            .set_charset(Some("utf8"))
            .create()
            .unwrap(),
    )
    .await;
}

#[tokio::test]
async fn testColumnCharsetChange() {
    let Some(connection) = connection("testColumnCharsetChange").await else {
        return;
    };
    assert_charset_changes_to_ascii(
        &connection,
        "test_column_charset_change",
        column("col_string", Types::STRING)
            .set_charset(Some("utf8"))
            .set_length(Some(100))
            .create()
            .unwrap(),
    )
    .await;
}

#[tokio::test]
async fn testColumnCollation() {
    let Some(connection) = connection("testColumnCollation").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_collation", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("text", Types::TEXT).create().unwrap(),
            column("foo", Types::TEXT)
                .set_collation(Some("latin1_swedish_ci"))
                .create()
                .unwrap(),
            column("bar", Types::TEXT)
                .set_collation(Some("utf8mb4_general_ci"))
                .create()
                .unwrap(),
            column("baz", Types::TEXT)
                .set_collation(Some("binary"))
                .create()
                .unwrap(),
        ])
        .set_options(options(&[
            ("charset", "latin1"),
            ("collation", "latin1_swedish_ci"),
        ]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_collation", None)
        .await
        .unwrap();

    assert_eq!(None, columns[0].get_collation());
    assert_eq!(
        Some("latin1_swedish_ci".to_owned()),
        columns[1].get_collation()
    );
    assert_eq!(
        Some("latin1_swedish_ci".to_owned()),
        columns[2].get_collation()
    );
    assert_eq!(
        Some("utf8mb4_general_ci".to_owned()),
        columns[3].get_collation()
    );
    assert_eq!(Types::BLOB, type_name(&columns[4]));
}

#[tokio::test]
async fn testListLobTypeColumns() {
    let Some(connection) = connection("testListLobTypeColumns").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let lob = |name: &str, type_name: &str, length: Option<i64>| {
        column(name, type_name).set_length(length).create().unwrap()
    };
    let table = Table::editor()
        .set_unquoted_name("lob_type_columns", None)
        .set_columns(vec![
            lob("col_tinytext", Types::TEXT, Some(LENGTH_LIMIT_TINYTEXT)),
            lob("col_text", Types::TEXT, Some(LENGTH_LIMIT_TEXT)),
            lob("col_mediumtext", Types::TEXT, Some(LENGTH_LIMIT_MEDIUMTEXT)),
            lob("col_longtext", Types::TEXT, None),
            lob("col_tinyblob", Types::TEXT, Some(LENGTH_LIMIT_TINYBLOB)),
            lob("col_blob", Types::BLOB, Some(LENGTH_LIMIT_BLOB)),
            lob("col_mediumblob", Types::BLOB, Some(LENGTH_LIMIT_MEDIUMBLOB)),
            lob("col_longblob", Types::BLOB, None),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let platform = platform(&connection).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("lob_type_columns", None)
        .await
        .unwrap();

    let names = [
        "col_tinytext",
        "col_text",
        "col_mediumtext",
        "col_longtext",
        "col_tinyblob",
        "col_blob",
        "col_mediumblob",
        "col_longblob",
    ];
    for (index, (name, online)) in names.iter().zip(&columns).enumerate() {
        let offline = table.get_column(name).unwrap().to_array(false).unwrap();
        let online = online.to_array(false).unwrap();
        if index < 4 {
            assert_eq!(
                platform.get_clob_type_declaration_sql(&offline).unwrap(),
                platform.get_clob_type_declaration_sql(&online).unwrap(),
                "{name}"
            );
        } else {
            assert_eq!(
                platform.get_blob_type_declaration_sql(&offline).unwrap(),
                platform.get_blob_type_declaration_sql(&online).unwrap(),
                "{name}"
            );
        }
    }
}

/// `$this->schemaManager->createComparator()->compareTables($online,
/// $offline)->isEmpty()` after creating `$offline`.
async fn assert_no_diff_after_create(connection: &Connection, offline_table: &Table, name: &str) {
    let sm = schema_manager(connection).await;
    drop_and_create_table(connection, offline_table).await;

    let online_table = sm
        .introspect_table_by_unquoted_name(name, None)
        .await
        .unwrap();

    assert!(
        sm.create_comparator()
            .await
            .unwrap()
            .compare_tables(&online_table, offline_table)
            .unwrap()
            .is_empty(),
        "No differences should be detected with the offline vs online schema."
    );
}

#[tokio::test]
async fn testDiffListGuidTableColumn() {
    let Some(connection) = connection("testDiffListGuidTableColumn").await else {
        return;
    };
    let offline_table = Table::editor()
        .set_unquoted_name("list_guid_table_column", None)
        .set_columns(vec![column("col_guid", Types::GUID).create().unwrap()])
        .create()
        .unwrap();

    assert_no_diff_after_create(&connection, &offline_table, "list_guid_table_column").await;
}

#[tokio::test]
async fn testListDecimalTypeColumns() {
    let Some(connection) = connection("testListDecimalTypeColumns").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_list_decimal_columns", None)
        .set_columns(vec![
            column("col", Types::DECIMAL)
                .set_precision(Some(10))
                .set_scale(6)
                .create()
                .unwrap(),
            column("col_unsigned", Types::DECIMAL)
                .set_precision(Some(10))
                .set_scale(6)
                .set_unsigned(true)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_list_decimal_columns", None)
        .await
        .unwrap();

    assert!(!columns[0].get_unsigned());
    assert!(columns[1].get_unsigned());
}

#[tokio::test]
async fn testListUnsignedFloatTypeColumns() {
    let Some(connection) = connection("testListUnsignedFloatTypeColumns").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_unsigned_float_columns", None)
        .set_columns(vec![
            column("col_unsigned", Types::FLOAT)
                .set_unsigned(true)
                .create()
                .unwrap(),
            column("col_smallfloat_unsigned", Types::SMALLFLOAT)
                .set_unsigned(true)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_unsigned_float_columns", None)
        .await
        .unwrap();

    assert_eq!(Types::FLOAT, type_name(&columns[0]));
    assert_eq!(Types::SMALLFLOAT, type_name(&columns[1]));
    assert!(columns[0].get_unsigned());
    assert!(columns[1].get_unsigned());
}

#[tokio::test]
async fn testConfigurableLengthColumns() {
    let Some(connection) = connection("testConfigurableLengthColumns").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_configurable_length_columns", None)
        .set_columns(vec![
            column("col_binary", Types::BINARY)
                .set_length(Some(16))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_configurable_length_columns", None)
        .await
        .unwrap();

    assert_eq!(Types::BINARY, type_name(&columns[0]));
    assert_eq!(Some(16), columns[0].get_length());
}

#[tokio::test]
async fn testJsonColumnType() {
    let Some(connection) = connection("testJsonColumnType").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let table = Table::editor()
        .set_unquoted_name("test_mysql_json", None)
        .set_columns(vec![column("col_json", Types::JSON).create().unwrap()])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let columns = sm
        .introspect_table_columns_by_unquoted_name("test_mysql_json", None)
        .await
        .unwrap();

    assert_eq!(Types::JSON, type_name(&columns[0]));
}

#[tokio::test]
async fn testColumnDefaultCurrentTimestamp() {
    let Some(connection) = connection("testColumnDefaultCurrentTimestamp").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let platform = platform(&connection).await;

    let current_time_stamp_sql = CurrentTimestamp.to_sql(platform.as_dyn());

    let table = Table::editor()
        .set_unquoted_name("test_column_defaults_current_timestamp", None)
        .set_columns(vec![
            column("col_datetime", Types::DATETIME_MUTABLE)
                .set_default_value(CurrentTimestamp::value())
                .create()
                .unwrap(),
            column("col_datetime_nullable", Types::DATETIME_MUTABLE)
                .set_default_value(CurrentTimestamp::value())
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let online_table = sm
        .introspect_table_by_unquoted_name("test_column_defaults_current_timestamp", None)
        .await
        .unwrap();
    assert_same_string(
        &current_time_stamp_sql,
        online_table
            .get_column("col_datetime")
            .unwrap()
            .get_default(),
    );
    assert_same_string(
        &current_time_stamp_sql,
        online_table
            .get_column("col_datetime_nullable")
            .unwrap()
            .get_default(),
    );

    assert!(
        sm.create_comparator()
            .await
            .unwrap()
            .compare_tables(&online_table, &table)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn testColumnDefaultsAreValid() {
    let Some(connection) = connection("testColumnDefaultsAreValid").await else {
        return;
    };
    let table = Table::editor()
        .set_unquoted_name("test_column_defaults_are_valid", None)
        .set_columns(vec![
            column("col_datetime_null", Types::DATETIME_MUTABLE)
                .set_not_null(false)
                .create()
                .unwrap(),
            column("col_int", Types::INTEGER)
                .set_default_value(Value::Int(1))
                .create()
                .unwrap(),
            column("col_neg_int", Types::INTEGER)
                .set_default_value(Value::Int(-1))
                .create()
                .unwrap(),
            column("col_string", Types::STRING)
                .set_length(Some(1))
                .set_default_value(Value::string("A"))
                .create()
                .unwrap(),
            column("col_decimal", Types::DECIMAL)
                .set_precision(Some(6))
                .set_scale(3)
                .set_default_value(Value::Float(-2.3))
                .create()
                .unwrap(),
            column("col_date", Types::DATE_MUTABLE)
                .set_default_value(Value::string("2012-12-12"))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    exec(
        &connection,
        "INSERT INTO test_column_defaults_are_valid () VALUES()",
    )
    .await;

    let row = connection
        .fetch_associative(
            "SELECT * FROM test_column_defaults_are_valid",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap()
        .expect("a row");

    let get = |key: &str| row.get(&Key::from(key)).cloned().unwrap_or(Value::Null);
    assert!(get("col_datetime_null").is_null());
    assert_same_string("2012-12-12", &get("col_date"));
    assert_same_string("A", &get("col_string"));
    assert!(phpunit_equals(&Value::Int(1), &get("col_int")));
    assert!(phpunit_equals(&Value::Int(-1), &get("col_neg_int")));
    assert!(phpunit_equals(
        &Value::string("-2.300"),
        &get("col_decimal")
    ));
}

#[tokio::test]
async fn testColumnDefaultValuesCurrentTimeAndDate() {
    let Some(connection) = connection("testColumnDefaultValuesCurrentTimeAndDate").await else {
        return;
    };
    let platform = platform(&connection).await;
    if !is_mariadb(platform) {
        // Only relevant for MariaDB.
        return;
    }
    let sm = schema_manager(&connection).await;

    let current_timestamp_sql = platform.get_current_timestamp_sql();
    let current_time_sql = platform.get_current_time_sql();
    let current_date_sql = platform.get_current_date_sql();

    let table = Table::editor()
        .set_unquoted_name("test_column_defaults_current_time_and_date", None)
        .set_columns(vec![
            column("col_datetime", Types::DATETIME_MUTABLE)
                .set_default_value(Value::string(current_timestamp_sql.as_str()))
                .create()
                .unwrap(),
            column("col_date", Types::DATE_MUTABLE)
                .set_default_value(Value::string(current_date_sql.as_str()))
                .create()
                .unwrap(),
            column("col_time", Types::TIME_MUTABLE)
                .set_default_value(Value::string(current_time_sql.as_str()))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let online_table = sm
        .introspect_table_by_unquoted_name("test_column_defaults_current_time_and_date", None)
        .await
        .unwrap();

    assert_same_string(
        &current_timestamp_sql,
        online_table
            .get_column("col_datetime")
            .unwrap()
            .get_default(),
    );
    assert_same_string(
        &current_date_sql,
        online_table.get_column("col_date").unwrap().get_default(),
    );
    assert_same_string(
        &current_time_sql,
        online_table.get_column("col_time").unwrap().get_default(),
    );

    assert!(
        sm.create_comparator()
            .await
            .unwrap()
            .compare_tables(&online_table, &table)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn testEnsureTableOptionsAreReflectedInMetadata() {
    let Some(connection) = connection("testEnsureTableOptionsAreReflectedInMetadata").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    exec(&connection, "DROP TABLE IF EXISTS test_table_metadata").await;

    let sql = "CREATE TABLE test_table_metadata(
  col1 INT NOT NULL AUTO_INCREMENT PRIMARY KEY
)
COLLATE utf8mb4_general_ci
ENGINE InnoDB
ROW_FORMAT DYNAMIC
COMMENT 'This is a test'
AUTO_INCREMENT=42
PARTITION BY HASH (col1)";

    exec(&connection, sql).await;
    let online_table = sm
        .introspect_table_by_unquoted_name("test_table_metadata", None)
        .await
        .unwrap();

    assert!(phpunit_equals(
        &Value::string("InnoDB"),
        &online_table.get_option("engine")
    ));
    assert!(phpunit_equals(
        &Value::string("utf8mb4_general_ci"),
        &online_table.get_option("collation")
    ));
    assert!(phpunit_equals(
        &Value::Int(42),
        &online_table.get_option("autoincrement")
    ));
    assert!(phpunit_equals(
        &Value::string("This is a test"),
        &online_table.get_option("comment")
    ));
    let mut create_options = PhpArray::new();
    create_options.insert("row_format", Value::string("DYNAMIC"));
    create_options.insert("partitioned", Value::Bool(true));
    assert!(
        phpunit_equals(
            &Value::Array(create_options),
            &online_table.get_option("create_options")
        ),
        "{:?}",
        online_table.get_option("create_options")
    );
}

#[tokio::test]
async fn testEnsureTableWithoutOptionsAreReflectedInMetadata() {
    let Some(connection) = connection("testEnsureTableWithoutOptionsAreReflectedInMetadata").await
    else {
        return;
    };
    let sm = schema_manager(&connection).await;
    exec(
        &connection,
        "DROP TABLE IF EXISTS test_table_empty_metadata",
    )
    .await;

    exec(
        &connection,
        "CREATE TABLE test_table_empty_metadata(col1 INT NOT NULL)",
    )
    .await;
    let online_table = sm
        .introspect_table_by_unquoted_name("test_table_empty_metadata", None)
        .await
        .unwrap();

    assert!(online_table.get_option("engine").is_truthy());
    // collation could be set to default or not set, information_schema indicate a possibly null value
    assert!(!online_table.has_option("autoincrement"));
    assert!(phpunit_equals(
        &Value::string(""),
        &online_table.get_option("comment")
    ));
    assert!(phpunit_equals(
        &Value::Array(PhpArray::new()),
        &online_table.get_option("create_options")
    ));
}

#[tokio::test]
async fn testColumnIntrospection() {
    let Some(connection) = connection("testColumnIntrospection").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    let mut table_editor = Table::editor().set_unquoted_name("test_column_introspection", None);

    for (type_, _) in types::get_types_map() {
        let column_editor = column(&format!("col_{type_}"), &type_);
        let column_editor = if type_ == Types::ENUM {
            column_editor.set_values(PhpArray::list([Value::string("foo"), Value::string("bar")]))
        } else {
            column_editor
                .set_length(Some(8))
                .set_precision(Some(8))
                .set_scale(2)
        };
        table_editor = table_editor.add_column(column_editor.create().unwrap());
    }

    let table = table_editor.create().unwrap();

    drop_and_create_table(&connection, &table).await;

    let online_table = sm
        .introspect_table_by_unquoted_name("test_column_introspection", None)
        .await
        .unwrap();

    let diff = sm
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(&online_table, &table)
        .unwrap();

    assert!(diff.is_empty(), "Tables should be identical.");
}

#[tokio::test]
async fn testListTableColumnsThrowsDatabaseRequired() {
    let Some(mut params) = mysql_support::params("testListTableColumnsThrowsDatabaseRequired")
    else {
        return;
    };
    params.remove(&Key::from("dbname"));

    let connection = DriverManager::get_connection(params, None).unwrap();
    let schema_manager = connection.create_schema_manager().await.unwrap();

    let e = schema_manager
        .introspect_table_columns_by_unquoted_name("users", None)
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::DATABASE_REQUIRED), "{e}");
}

#[tokio::test]
async fn testSchemaDiffWithCustomColumnTypeWhileDatabaseTypeDiffers() {
    let Some(connection) =
        connection("testSchemaDiffWithCustomColumnTypeWhileDatabaseTypeDiffers").await
    else {
        return;
    };
    let sm = schema_manager(&connection).await;
    if !types::has_type("custom_type") {
        types::add_type("custom_type", TypeArgument::Instance(Arc::new(CustomType))).unwrap();
    }

    let metadata_table = Table::editor()
        .set_unquoted_name("table_with_custom_type", None)
        .set_columns(vec![column("col1", "custom_type").create().unwrap()])
        .create()
        .unwrap();

    let platform = platform(&connection).await;
    assert_eq!(
        vec!["CREATE TABLE table_with_custom_type (col1 INT NOT NULL)".to_owned()],
        platform.get_create_table_sql(&metadata_table).unwrap()
    );

    exec(&connection, "DROP TABLE IF EXISTS table_with_custom_type").await;

    exec(
        &connection,
        "CREATE TABLE table_with_custom_type (col1 VARCHAR(255) NOT NULL)",
    )
    .await;
    let online_table = sm.introspect_table("table_with_custom_type").await.unwrap();

    let comparator = sm.create_comparator().await.unwrap();
    let tables_diff = comparator
        .compare_tables(&online_table, &metadata_table)
        .unwrap();

    assert_eq!(
        vec!["ALTER TABLE table_with_custom_type CHANGE col1 col1 INT NOT NULL".to_owned()],
        platform.get_alter_table_sql(&tables_diff).unwrap(),
        " The column should be changed from VARCHAR TO INT"
    );
}

#[tokio::test]
async fn testMigrateOldBoolean() {
    let Some(connection) = connection("testMigrateOldBoolean").await else {
        return;
    };
    let sm = schema_manager(&connection).await;
    exec(&connection, "DROP TABLE IF EXISTS table_with_old_boolean").await;
    exec(
        &connection,
        "CREATE TABLE table_with_old_boolean (val TINYINT(1) NOT NULL)",
    )
    .await;

    let online_table = sm
        .introspect_table_by_unquoted_name("table_with_old_boolean", None)
        .await
        .unwrap();
    let target_table = Table::editor()
        .set_unquoted_name("table_with_old_boolean", None)
        .set_columns(vec![column("val", Types::BOOLEAN).create().unwrap()])
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(&online_table, &target_table)
        .unwrap();

    assert!(diff.is_empty(), "Tables should be identical.");
}
