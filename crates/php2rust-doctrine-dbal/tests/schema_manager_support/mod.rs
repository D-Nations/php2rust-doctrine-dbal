//! Helpers of the schema manager functional tests: what
//! tests/FunctionalTestCase.php and
//! tests/Functional/Schema/SchemaManagerFunctionalTestCase.php give their
//! tests, on `pdo_sqlite` in memory (a fresh database per test, so the
//! `tearDown()` drops have nothing to do).
//!
//! Upstream: none
#![allow(dead_code)]

use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::platforms::AbstractPlatform;
use php2rust_doctrine_dbal::schema::name::{OptionallyQualifiedName, UnqualifiedName};
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, Column, ColumnEditor, ForeignKeyConstraint, Index, IndexedColumn,
    PrimaryKeyConstraint, SchemaManager, Table,
};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Configuration, Connection, DriverManager, Exception, ParameterTypes};

/// `DriverManager::getConnection(['driver' => 'pdo_sqlite', 'memory' =>
/// true], $configuration)`.
pub fn sqlite_with(configuration: Option<Configuration>) -> Connection {
    let mut params = PhpArray::new();
    params.insert("driver", Value::string("pdo_sqlite"));
    params.insert("memory", Value::Bool(true));
    DriverManager::get_connection(params, configuration).unwrap()
}

/// The functional tests' connection.
pub fn sqlite() -> Connection {
    sqlite_with(None)
}

/// `$this->connection->createSchemaManager()`.
pub async fn schema_manager(connection: &Connection) -> SchemaManager<'_> {
    connection.create_schema_manager().await.unwrap()
}

/// `$this->connection->getDatabasePlatform()`.
pub async fn platform(connection: &Connection) -> &(dyn AbstractPlatform + Send + Sync) {
    connection.get_database_platform().await.unwrap()
}

/// `$this->connection->executeStatement($sql)`.
pub async fn exec(connection: &Connection, sql: &str) {
    connection
        .execute_statement(sql, &PhpArray::new(), &ParameterTypes::new())
        .await
        .unwrap();
}

/// `dropTableIfExists($name)`.
pub async fn drop_table_if_exists(connection: &Connection, name: &str) {
    match schema_manager(connection).await.drop_table(name).await {
        Ok(()) => {}
        Err(e) if e.is_instance_of(Exception::DATABASE_OBJECT_NOT_FOUND_EXCEPTION) => {}
        Err(e) => panic!("{e}"),
    }
}

/// `dropAndCreateTable($table)`.
pub async fn drop_and_create_table(connection: &Connection, table: &Table) {
    let name = table.get_quoted_name(platform(connection).await);
    drop_table_if_exists(connection, &name).await;
    schema_manager(connection)
        .await
        .create_table(table)
        .await
        .unwrap();
}

/// `Column::editor()->setUnquotedName($name)->setTypeName($type)`.
pub fn column(name: &str, type_name: &str) -> ColumnEditor {
    Column::editor()
        .set_unquoted_name(name)
        .set_type_name(type_name)
}

/// A primary key constraint on the unquoted column names, as
/// `setPrimaryKeyConstraint()` takes it.
#[allow(clippy::unnecessary_wraps, reason = "the editor takes an Option")]
pub fn primary_key(columns: &[&str]) -> Option<PrimaryKeyConstraint> {
    Some(
        PrimaryKeyConstraint::editor()
            .set_unquoted_column_names(columns)
            .create()
            .unwrap(),
    )
}

/// An index on the unquoted column names.
pub fn index(name: &str, columns: &[&str]) -> Index {
    Index::editor()
        .set_unquoted_name(name)
        .set_unquoted_column_names(columns)
        .create()
        .unwrap()
}

/// A foreign key constraint between unquoted names.
pub fn foreign_key(
    name: Option<&str>,
    local: &[&str],
    foreign_table: &str,
    foreign: &[&str],
) -> ForeignKeyConstraint {
    let mut editor = ForeignKeyConstraint::editor();
    if let Some(name) = name {
        editor = editor.set_unquoted_name(name);
    }
    editor
        .set_unquoted_referencing_column_names(local)
        .set_unquoted_referenced_table_name(foreign_table, None)
        .set_unquoted_referenced_column_names(foreign)
        .create()
        .unwrap()
}

/// `getTestTable($unquotedName, $options)`.
pub async fn get_test_table(
    schema_manager: &SchemaManager<'_>,
    name: &str,
    options: PhpArray,
) -> Table {
    Table::editor()
        .set_unquoted_name(name, None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("test", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
            column("foreign_key_test", Types::INTEGER)
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_options(options)
        .set_configuration(
            schema_manager
                .create_schema_config()
                .await
                .unwrap()
                .to_table_configuration(),
        )
        .create()
        .unwrap()
}

/// `createTestTable($name)`.
pub async fn create_test_table(connection: &Connection, name: &str) -> Table {
    let table = get_test_table(&schema_manager(connection).await, name, PhpArray::new()).await;
    drop_and_create_table(connection, &table).await;
    table
}

/// `getTestCompositeTable($unquotedName)`.
pub async fn get_test_composite_table(schema_manager: &SchemaManager<'_>, name: &str) -> Table {
    Table::editor()
        .set_unquoted_name(name, None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("other_id", Types::INTEGER).create().unwrap(),
            column("test", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id", "other_id"]))
        .set_configuration(
            schema_manager
                .create_schema_config()
                .await
                .unwrap()
                .to_table_configuration(),
        )
        .create()
        .unwrap()
}

/// `assertUnqualifiedNameEquals(UnqualifiedName::unquoted($expected),
/// $actual)`: `SQLite` does not fold unquoted names, so both quote to the
/// same value.
pub fn assert_unqualified_name_equals(expected: &str, actual: &UnqualifiedName) {
    assert_eq!(expected, actual.get_identifier().get_value());
}

/// `findObjectByShortestName($objects, $name)` of tables.
pub fn find_table_by_shortest_name<'t>(tables: &'t [Table], name: &str) -> Option<&'t Table> {
    tables
        .iter()
        .find(|t| t.get_shortest_name(t.get_namespace_name()) == name)
}

/// `array_map('strtolower', $names)`.
pub fn lower(names: Vec<String>) -> Vec<String> {
    names.into_iter().map(|n| n.to_lowercase()).collect()
}

/// `toQuotedUnqualifiedName($name)`: `SQLite` does not fold.
pub fn to_quoted_unqualified_name(name: &UnqualifiedName) -> UnqualifiedName {
    UnqualifiedName::quoted(name.get_identifier().get_value()).unwrap()
}

/// `toQuotedIndex($index)`.
pub fn to_quoted_index(index: &Index) -> Index {
    let columns = index
        .get_indexed_columns()
        .unwrap()
        .iter()
        .map(|column| {
            IndexedColumn::new(
                to_quoted_unqualified_name(column.get_column_name()),
                column.get_length(),
            )
            .unwrap()
        })
        .collect();
    index
        .edit()
        .unwrap()
        .set_name(Some(to_quoted_unqualified_name(
            &index.get_object_name().unwrap(),
        )))
        .set_columns(columns)
        .create()
        .unwrap()
}

/// `assertIndexEquals($expected, $actual)`.
pub fn assert_index_equals(expected: &Index, actual: &Index) {
    assert_eq!(to_quoted_index(expected), to_quoted_index(actual));
}

/// `assertIndexListEquals($expected, $actual)`: sorted by name.
pub fn assert_index_list_equals(expected: &[Index], actual: &[Index]) {
    let sorted = |indexes: &[Index]| {
        let mut quoted: Vec<Index> = indexes.iter().map(to_quoted_index).collect();
        quoted.sort_by_key(|index| {
            index
                .get_object_name()
                .unwrap()
                .get_identifier()
                .get_value()
                .to_owned()
        });
        quoted
    };
    assert_eq!(sorted(expected), sorted(actual));
}

/// `toQuotedForeignKeyConstraint($constraint)`.
pub fn to_quoted_foreign_key(constraint: &ForeignKeyConstraint) -> ForeignKeyConstraint {
    let names = |names: &[UnqualifiedName]| names.iter().map(to_quoted_unqualified_name).collect();
    let table = constraint.get_referenced_table_name().unwrap();
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
        .set_referencing_column_names(names(constraint.get_referencing_column_names().unwrap()))
        .set_referenced_table_name(
            OptionallyQualifiedName::quoted(
                table.get_unqualified_name().get_value(),
                table
                    .get_qualifier()
                    .map(php2rust_doctrine_dbal::schema::name::Identifier::get_value),
            )
            .unwrap(),
        )
        .set_referenced_column_names(names(constraint.get_referenced_column_names().unwrap()))
        .create()
        .unwrap()
}

/// `assertForeignKeyConstraintListEquals($expected, $actual)`.
pub fn assert_foreign_key_constraint_list_equals(
    expected: &[&ForeignKeyConstraint],
    actual: &[&ForeignKeyConstraint],
) {
    let quoted = |list: &[&ForeignKeyConstraint]| -> Vec<ForeignKeyConstraint> {
        list.iter().map(|fk| to_quoted_foreign_key(fk)).collect()
    };
    assert_eq!(quoted(expected), quoted(actual));
}

/// The `ForeignKeyConstraint` fields `assertEquals()` compares.
pub fn assert_foreign_key_equals(expected: &ForeignKeyConstraint, actual: &ForeignKeyConstraint) {
    assert_eq!(expected.get_name(), actual.get_name());
    assert_eq!(expected.get_local_columns(), actual.get_local_columns());
    assert_eq!(
        expected.get_foreign_table_name(),
        actual.get_foreign_table_name()
    );
    assert_eq!(expected.get_foreign_columns(), actual.get_foreign_columns());
    assert_eq!(expected.get_options().len(), actual.get_options().len());
    for (key, value) in expected.get_options().iter() {
        let key = match key {
            Key::Str(key) => key.to_string_lossy().into_owned(),
            Key::Int(key) => key.to_string(),
        };
        assert!(
            value.identical(&actual.get_option(&key)) || value_eq(value, &actual.get_option(&key)),
            "option {key}: {value:?} != {:?}",
            actual.get_option(&key)
        );
    }
}

fn value_eq(a: &Value, b: &Value) -> bool {
    php_value::vectors::phpunit_equals(a, b)
}
