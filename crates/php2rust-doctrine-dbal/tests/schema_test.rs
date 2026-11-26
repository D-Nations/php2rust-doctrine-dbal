//! Port of tests/Schema/SchemaTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. `SchemaEditor` returns the first error of a modification
//! from `create()` (PHP throws from the modifying call). The schema owns
//! its tables (PHP shares the caller's handles), so `assertSame()` on a
//! table compares the tables a round trip through `edit()` gives, and
//! `assertNotSame()` after `clone` compares addresses.
//!
//! Upstream: doctrine/dbal tests/Schema/SchemaTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php_value::PhpArray;
use php2rust_doctrine_dbal::schema::name::OptionallyQualifiedName;
use php2rust_doctrine_dbal::schema::{
    Column, ForeignKeyConstraint, Schema, SchemaConfig, Sequence, Table,
};
use php2rust_doctrine_dbal::types::Types;
use schema_support::{expect_deprecation, expect_exception, expect_no_deprecation};

const SCHEMA_EXCEPTION: &str = "Doctrine\\DBAL\\Schema\\SchemaException";
const QUALIFIED_NAMES: &str =
    "https://github.com/doctrine/dbal/pull/6677#user-content-qualified-names";
const UNQUALIFIED_NAMES: &str =
    "https://github.com/doctrine/dbal/pull/6677#user-content-unqualified-names";

fn id_column() -> Column {
    Column::editor()
        .set_unquoted_name("id")
        .set_type_name(Types::INTEGER)
        .create()
        .unwrap()
}

fn create_table(unqualified_name: &str, qualifier: Option<&str>) -> Table {
    Table::editor()
        .set_unquoted_name(unqualified_name, qualifier)
        .set_columns(vec![id_column()])
        .create()
        .unwrap()
}

fn create_sequence(unqualified_name: &str, qualifier: Option<&str>) -> Sequence {
    Sequence::editor()
        .set_unquoted_name(unqualified_name, qualifier)
        .create()
        .unwrap()
}

/// `assertSame($expected, $actual)` on tables.
fn assert_same_table(expected: &Table, actual: &Table) {
    assert_eq!(
        format!("{:?}", expected.edit().unwrap().create().unwrap()),
        format!("{:?}", actual.edit().unwrap().create().unwrap())
    );
}

fn assert_same_tables(expected: &[&Table], actual: &[&Table]) {
    assert_eq!(expected.len(), actual.len());
    for (expected, actual) in expected.iter().zip(actual) {
        assert_same_table(expected, actual);
    }
}

#[test]
fn testAddTable() {
    let table_name = "public.foo";
    let table = Table::editor()
        .set_unquoted_name("foo", Some("public"))
        .set_columns(vec![id_column()])
        .create()
        .unwrap();

    let schema = Schema::editor().add_table(table.clone()).create().unwrap();

    assert!(schema.has_table(table_name));

    assert_same_tables(&[&table], &schema.get_tables());
    assert_same_table(&table, schema.get_table(table_name).unwrap());
    assert!(schema.has_table(table_name));
}

#[test]
fn testTableMatchingCaseInsensitive() {
    let table = create_table("Foo", None);

    let schema = Schema::editor().add_table(table.clone()).create().unwrap();

    assert!(schema.has_table("foo"));
    assert!(schema.has_table("FOO"));

    assert_same_table(&table, schema.get_table("FOO").unwrap());
    assert_same_table(&table, schema.get_table("foo").unwrap());
    assert_same_table(&table, schema.get_table("Foo").unwrap());
}

#[test]
fn testGetUnknownTableThrowsException() {
    let schema = Schema::editor().create().unwrap();

    expect_exception(SCHEMA_EXCEPTION, schema.get_table("unknown"));
}

#[test]
fn testCreateTableTwiceThrowsException() {
    let table = create_table("foo", None);
    let tables = vec![table.clone(), table];

    expect_exception(SCHEMA_EXCEPTION, Schema::new(tables, vec![], None, &[]));
}

#[test]
fn testRenameTable() {
    let table = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![id_column()])
        .create()
        .unwrap();

    let mut schema = Schema::editor().add_table(table.clone()).create().unwrap();

    assert!(schema.has_table("foo"));
    schema.rename_table("foo", "bar").unwrap();
    assert!(!schema.has_table("foo"));
    assert!(schema.has_table("bar"));
    assert_same_table(&table, schema.get_table("bar").unwrap());
}

#[test]
fn testDropTable() {
    let table = create_table("foo", None);

    let mut schema = Schema::editor().add_table(table).create().unwrap();

    assert!(schema.has_table("foo"));

    schema.drop_table("foo").unwrap();

    assert!(!schema.has_table("foo"));
}

#[test]
fn testCreateTable() {
    let mut schema = Schema::editor().create().unwrap();

    assert!(!schema.has_table("foo"));

    let table = schema.create_table("foo").unwrap();

    assert_eq!(
        OptionallyQualifiedName::unquoted("foo", None).unwrap(),
        table.get_object_name().unwrap()
    );
    assert!(schema.has_table("foo"));
}

#[test]
fn testAddSequences() {
    let sequence = create_sequence("a_seq", None);

    let schema = Schema::editor()
        .add_sequence(sequence.clone())
        .create()
        .unwrap();

    assert!(schema.has_sequence("a_seq"));
    assert_eq!(
        OptionallyQualifiedName::unquoted("a_seq", None).unwrap(),
        schema
            .get_sequence("a_seq")
            .unwrap()
            .get_object_name()
            .unwrap()
    );

    assert_eq!(vec![&sequence], schema.get_sequences());
}

#[test]
fn testSequenceAccessCaseInsensitive() {
    let sequence = create_sequence("a_Seq", None);

    let schema = Schema::editor()
        .add_sequence(sequence.clone())
        .create()
        .unwrap();

    assert!(schema.has_sequence("a_seq"));
    assert!(schema.has_sequence("a_Seq"));
    assert!(schema.has_sequence("A_SEQ"));

    assert_eq!(&sequence, schema.get_sequence("a_seq").unwrap());
    assert_eq!(&sequence, schema.get_sequence("a_Seq").unwrap());
    assert_eq!(&sequence, schema.get_sequence("A_SEQ").unwrap());
}

#[test]
fn testGetUnknownSequenceThrowsException() {
    let schema = Schema::editor().create().unwrap();

    expect_exception(SCHEMA_EXCEPTION, schema.get_sequence("unknown"));
}

#[test]
fn testCreateSequence() {
    let mut schema = Schema::editor().create().unwrap();

    let sequence = schema.create_sequence("a_seq", 10, 20).unwrap().clone();

    assert_eq!(
        OptionallyQualifiedName::unquoted("a_seq", None).unwrap(),
        sequence.get_object_name().unwrap()
    );
    assert_eq!(10, sequence.get_allocation_size());
    assert_eq!(20, sequence.get_initial_value());

    assert!(schema.has_sequence("a_seq"));
    assert_eq!(
        OptionallyQualifiedName::unquoted("a_seq", None).unwrap(),
        schema
            .get_sequence("a_seq")
            .unwrap()
            .get_object_name()
            .unwrap()
    );

    assert_eq!(vec![&sequence], schema.get_sequences());
}

#[test]
fn testDropSequence() {
    let sequence = create_sequence("a_seq", None);

    let mut schema = Schema::editor().add_sequence(sequence).create().unwrap();

    schema.drop_sequence("a_seq");
    assert!(!schema.has_sequence("a_seq"));
}

#[test]
fn testAddSequenceTwiceThrowsException() {
    let sequence = create_sequence("a_seq", None);

    expect_exception(
        SCHEMA_EXCEPTION,
        Schema::new(vec![], vec![sequence.clone(), sequence], None, &[]),
    );
}

#[test]
fn testConfigMaxIdentifierLength() {
    let mut schema_config = SchemaConfig::new();
    schema_config.set_max_identifier_length(5);

    let mut schema = Schema::new(vec![], vec![], Some(schema_config), &[]).unwrap();

    let table = schema.create_table("smalltable").unwrap();
    table
        .add_column("long_id", Types::INTEGER, &PhpArray::new())
        .unwrap();
    table
        .add_index(["long_id"], None, vec![], PhpArray::new())
        .unwrap();

    let indexes = table.get_indexes();
    assert_eq!(1, indexes.len());

    let index = indexes.values().next();
    let index = index.expect("assertNotNull($index)");
    assert_eq!(5, index.get_object_name().unwrap().to_string().len());
}

#[test]
fn testDeepClone() {
    let table_a = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![id_column()])
        .create()
        .unwrap();

    let table_b = Table::editor()
        .set_unquoted_name("bar", None)
        .set_columns(vec![
            id_column(),
            Column::editor()
                .set_unquoted_name("foo_id")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
        ])
        .set_foreign_key_constraints(vec![
            ForeignKeyConstraint::editor()
                .set_unquoted_referencing_column_names(&["foo_id"])
                .set_unquoted_referenced_table_name("foo", None)
                .set_unquoted_referenced_column_names(&["id"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let sequence = Sequence::editor()
        .set_unquoted_name("baz", None)
        .create()
        .unwrap();

    let schema = Schema::editor()
        .set_tables(vec![table_a, table_b])
        .add_sequence(sequence)
        .create()
        .unwrap();

    let schema_new = schema.clone();

    assert!(!std::ptr::eq(
        schema.get_sequence("baz").unwrap(),
        schema_new.get_sequence("baz").unwrap()
    ));

    let (foo, foo_new) = (
        schema.get_table("foo").unwrap(),
        schema_new.get_table("foo").unwrap(),
    );
    assert!(!std::ptr::eq(foo, foo_new));
    assert!(!std::ptr::eq(
        foo.get_column("id").unwrap(),
        foo_new.get_column("id").unwrap()
    ));

    let (bar, bar_new) = (
        schema.get_table("bar").unwrap(),
        schema_new.get_table("bar").unwrap(),
    );
    assert!(!std::ptr::eq(bar, bar_new));
    assert!(!std::ptr::eq(
        bar.get_column("id").unwrap(),
        bar_new.get_column("id").unwrap()
    ));
}

#[test]
fn testHasTableForQuotedAsset() {
    let table_a = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![id_column()])
        .create()
        .unwrap();

    let schema = Schema::editor().add_table(table_a).create().unwrap();

    assert!(schema.has_table("`foo`"));
}

#[test]
fn testHasNamespace() {
    let schema = Schema::editor()
        .set_default_namespace(Some("public"))
        .create()
        .unwrap();

    assert!(!schema.has_namespace("foo"));

    let schema = schema
        .edit()
        .add_table(create_table("foo", None))
        .create()
        .unwrap();

    assert!(!schema.has_namespace("foo"));

    let schema = schema
        .edit()
        .add_table(create_table("baz", Some("bar")))
        .create()
        .unwrap();

    assert!(!schema.has_namespace("baz"));
    assert!(schema.has_namespace("bar"));
    assert!(!schema.has_namespace("tab"));

    let schema = schema
        .edit()
        .add_table(create_table("taz", Some("tab")))
        .create()
        .unwrap();

    assert!(schema.has_namespace("tab"));
}

#[test]
fn testCreatesNamespace() {
    let mut schema = Schema::editor().create().unwrap();

    assert!(!schema.has_namespace("foo"));

    schema.create_namespace("foo").unwrap();

    assert!(schema.has_namespace("foo"));
    assert!(schema.has_namespace("FOO"));
    assert!(schema.has_namespace("`foo`"));
    assert!(schema.has_namespace("`FOO`"));

    schema.create_namespace("`bar`").unwrap();

    assert!(schema.has_namespace("bar"));
    assert!(schema.has_namespace("BAR"));
    assert!(schema.has_namespace("`bar`"));
    assert!(schema.has_namespace("`BAR`"));

    assert_eq!(vec!["foo", "`bar`"], schema.get_namespaces());
}

#[test]
fn testThrowsExceptionOnCreatingNamespaceTwice() {
    let mut schema = Schema::editor().create().unwrap();

    schema.create_namespace("foo").unwrap();

    expect_exception(SCHEMA_EXCEPTION, schema.create_namespace("foo").map(|_| ()));
}

#[test]
fn testCreatesNamespaceThroughAddingTableImplicitly() {
    let schema = Schema::editor()
        .set_default_namespace(Some("public"))
        .create()
        .unwrap();

    assert!(!schema.has_namespace("foo"));

    let schema = schema
        .edit()
        .add_table(create_table("baz", None))
        .create()
        .unwrap();

    assert!(!schema.has_namespace("foo"));
    assert!(!schema.has_namespace("baz"));

    let schema = schema
        .edit()
        .add_table(create_table("bar", Some("foo")))
        .create()
        .unwrap();

    assert!(schema.has_namespace("foo"));
    assert!(!schema.has_namespace("bar"));

    let schema = schema
        .edit()
        .add_table(create_table("bloo", Some("baz")))
        .create()
        .unwrap();

    assert!(schema.has_namespace("baz"));
    assert!(!schema.has_namespace("bloo"));

    let schema = schema
        .edit()
        .add_table(create_table("moo", Some("baz")))
        .create()
        .unwrap();

    assert!(schema.has_namespace("baz"));
    assert!(!schema.has_namespace("moo"));
}

#[test]
fn testCreatesNamespaceThroughAddingSequenceImplicitly() {
    let schema = Schema::editor()
        .set_default_namespace(Some("public"))
        .create()
        .unwrap();

    assert!(!schema.has_namespace("foo"));

    let schema = schema
        .edit()
        .add_sequence(create_sequence("baz", None))
        .create()
        .unwrap();

    assert!(!schema.has_namespace("foo"));
    assert!(!schema.has_namespace("baz"));

    let schema = schema
        .edit()
        .add_sequence(create_sequence("bar", Some("foo")))
        .create()
        .unwrap();

    assert!(schema.has_namespace("foo"));
    assert!(!schema.has_namespace("bar"));

    let schema = schema
        .edit()
        .add_sequence(create_sequence("bloo", Some("baz")))
        .create()
        .unwrap();

    assert!(schema.has_namespace("baz"));
    assert!(!schema.has_namespace("bloo"));

    let schema = schema
        .edit()
        .add_sequence(create_sequence("moo", Some("baz")))
        .create()
        .unwrap();

    assert!(schema.has_namespace("baz"));
    assert!(!schema.has_namespace("moo"));
}

#[test]
fn testAddObjectWithQualifiedNameAfterUnqualifiedName() {
    expect_deprecation(QUALIFIED_NAMES, || {
        Schema::new(
            vec![create_table("t", None), create_table("t", Some("public"))],
            vec![],
            None,
            &[],
        )
        .unwrap();
    });
}

#[test]
fn testAddObjectWithUnqualifiedNameAfterQualifiedName() {
    expect_deprecation(UNQUALIFIED_NAMES, || {
        Schema::new(
            vec![create_table("t", Some("public")), create_table("t", None)],
            vec![],
            None,
            &[],
        )
        .unwrap();
    });
}

#[test]
fn testReferenceByQualifiedNameAmongUnqualifiedNames() {
    let schema = Schema::editor()
        .add_table(create_table("t", None))
        .create()
        .unwrap();

    expect_deprecation(QUALIFIED_NAMES, || schema.has_table("public.t"));
}

#[test]
fn testReferenceByUnqualifiedNameAmongQualifiedNames() {
    let schema = Schema::editor()
        .add_table(create_table("t", Some("public")))
        .create()
        .unwrap();

    expect_deprecation(UNQUALIFIED_NAMES, || schema.has_table("t"));
}

#[test]
fn testAddObjectWithQualifiedNameAfterUnqualifiedNameWithDefaultNamespace() {
    let mut schema_config = SchemaConfig::new();
    schema_config.set_name(Some("public"));

    expect_no_deprecation(QUALIFIED_NAMES, || {
        Schema::new(
            vec![create_table("t", None), create_table("s", Some("public"))],
            vec![],
            Some(schema_config),
            &[],
        )
        .unwrap();
    });
}

#[test]
fn testAddObjectWithUnqualifiedNameAfterQualifiedNameWithDefaultNamespace() {
    let mut schema_config = SchemaConfig::new();
    schema_config.set_name(Some("public"));

    expect_no_deprecation(UNQUALIFIED_NAMES, || {
        Schema::new(
            vec![create_table("t", Some("public")), create_table("s", None)],
            vec![],
            Some(schema_config),
            &[],
        )
        .unwrap();
    });
}

#[test]
fn testReferencingByUnqualifiedNameAmongQualifiedNamesWithDefaultNamespace() {
    let schema = Schema::editor()
        .set_default_namespace(Some("public"))
        .add_table(create_table("t", Some("public")))
        .create()
        .unwrap();

    assert!(schema.has_table("t"));
    assert!(schema.has_table("public.t"));

    assert!(!schema.has_table("s"));
    assert!(!schema.has_table("public.s"));
}
