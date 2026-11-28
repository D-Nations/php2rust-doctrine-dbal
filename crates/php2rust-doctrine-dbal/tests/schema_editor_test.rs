//! Port of tests/Schema/SchemaEditorTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. `SchemaEditor` is a by-value builder that keeps the first
//! error of a modification and returns it from `create()` (PHP throws
//! from the modifying call), so `expectException()` before a modifying
//! call is checked on the `create()` that follows it. The schema owns
//! its tables, so `assertSame()` on a table compares the tables a round
//! trip through `edit()` gives.
//!
//! Upstream: doctrine/dbal tests/Schema/SchemaEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php2rust_doctrine_dbal::schema::name::{OptionallyQualifiedName, UnqualifiedName};
use php2rust_doctrine_dbal::schema::{
    Column, Index, IndexType, Schema, SchemaEditor, Sequence, Table, TableEditor,
};
use php2rust_doctrine_dbal::types::Types;
use schema_support::expect_exception;

fn create_table(unqualified_name: &str, qualifier: Option<&str>) -> Table {
    Table::editor()
        .set_unquoted_name(unqualified_name, qualifier)
        .set_columns(vec![
            Column::editor()
                .set_unquoted_name("id")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap()
}

fn sequence(name: &str) -> Sequence {
    Sequence::editor()
        .set_unquoted_name(name, None)
        .create()
        .unwrap()
}

/// `assertSame($expected, $actual)` on lists of tables.
fn assert_same_tables(expected: &[&Table], actual: &[&Table]) {
    let round_trip = |tables: &[&Table]| -> Vec<String> {
        tables
            .iter()
            .map(|table| format!("{:?}", table.edit().unwrap().create().unwrap()))
            .collect()
    };
    assert_eq!(round_trip(expected), round_trip(actual));
}

fn invalid_modification(editor: SchemaEditor) {
    expect_exception("InvalidSchemaModification", editor.create());
}

#[test]
fn testEditorCreatesEmptySchema() {
    let schema = Schema::editor().create().unwrap();

    assert!(schema.get_tables().is_empty());
    assert_eq!(
        schema.get_sequences(),
        [] as [&php2rust_doctrine_dbal::schema::Sequence; 0]
    );
}

#[test]
fn testEditRoundTripPreservesTablesAndSequences() {
    let table_a = create_table("foo", None);
    let table_b = create_table("bar", None);
    let sequence = sequence("a_seq");

    let schema = Schema::editor()
        .set_tables(vec![table_a.clone(), table_b.clone()])
        .add_sequence(sequence.clone())
        .create()
        .unwrap();

    let result = schema.edit().create().unwrap();

    assert_same_tables(&[&table_a, &table_b], &result.get_tables());
    assert_eq!(vec![&sequence], result.get_sequences());
}

#[test]
fn testEditRoundTripPropagatesDefaultNamespace() {
    let schema = Schema::editor()
        .set_default_namespace(Some("public"))
        .add_table(create_table("foo", None))
        .create()
        .unwrap();

    let result = schema.edit().create().unwrap();

    assert!(result.has_table("public.foo"));
    assert!(result.has_table("foo"));
}

#[test]
fn testAddTableRejectsDuplicateName() {
    let editor = Schema::editor().add_table(create_table("foo", None));

    invalid_modification(editor.add_table(create_table("foo", None)));
}

#[test]
fn testSetTablesReplacesPreviousTables() {
    let bar = create_table("bar", None);

    let schema = Schema::editor()
        .set_tables(vec![create_table("foo", None)])
        .set_tables(vec![bar.clone()])
        .create()
        .unwrap();

    assert!(!schema.has_table("foo"));
    assert_same_tables(&[&bar], &schema.get_tables());
}

#[test]
fn testSetSequencesReplacesPreviousSequences() {
    let bar = sequence("bar");

    let schema = Schema::editor()
        .set_sequences(vec![sequence("foo")])
        .set_sequences(vec![bar.clone()])
        .create()
        .unwrap();

    assert!(!schema.has_sequence("foo"));
    assert_eq!(vec![&bar], schema.get_sequences());
}

#[test]
fn testAddSequenceRejectsDuplicateName() {
    let editor = Schema::editor().add_sequence(sequence("a_seq"));

    invalid_modification(editor.add_sequence(sequence("a_seq")));
}

#[test]
fn testModifyTableReplacesInPlacePreservingOrder() {
    let schema = Schema::editor()
        .add_table(create_table("first", None))
        .add_table(create_table("second", None))
        .add_table(create_table("third", None))
        .modify_table_by_unquoted_name(
            "second",
            |editor: TableEditor| {
                editor.add_index(
                    Index::editor()
                        .set_unquoted_name("second_idx")
                        .set_unquoted_column_names(&["id"])
                        .create()
                        .unwrap(),
                )
            },
            None,
        )
        .create()
        .unwrap();

    assert_eq!(
        vec!["first", "second", "third"],
        schema
            .get_tables()
            .iter()
            .map(|table| table.get_object_name().unwrap().to_string())
            .collect::<Vec<_>>()
    );

    assert_eq!(1, schema.get_table("second").unwrap().get_indexes().len());
}

#[test]
fn testModifyTableAllowsRename() {
    let schema = Schema::editor()
        .add_table(create_table("foo", None))
        .modify_table_by_unquoted_name(
            "foo",
            |editor: TableEditor| editor.set_unquoted_name("renamed", None),
            None,
        )
        .create()
        .unwrap();

    assert!(schema.has_table("renamed"));
    assert!(!schema.has_table("foo"));
}

#[test]
fn testModifyTableOnAbsentNameThrows() {
    let editor = Schema::editor();

    invalid_modification(editor.modify_table_by_unquoted_name(
        "missing",
        |editor: TableEditor| editor,
        None,
    ));
}

#[test]
fn testModifyTableRenameCollisionThrows() {
    let editor = Schema::editor()
        .add_table(create_table("foo", None))
        .add_table(create_table("bar", None));

    invalid_modification(editor.modify_table_by_unquoted_name(
        "foo",
        |te: TableEditor| te.set_unquoted_name("bar", None),
        None,
    ));
}

#[test]
fn testModifyTableCannotChangeQualifier() {
    let editor = Schema::editor().add_table(create_table("t", Some("foo")));

    invalid_modification(editor.modify_table_by_unquoted_name(
        "t",
        |te: TableEditor| te.set_unquoted_name("t", Some("bar")),
        Some("foo"),
    ));
}

#[test]
fn testRenameTablePreservesQualifier() {
    let schema = Schema::editor()
        .add_table(create_table("foo", Some("public")))
        .rename_table(
            &OptionallyQualifiedName::unquoted("foo", Some("public")).unwrap(),
            &UnqualifiedName::unquoted("renamed").unwrap(),
        )
        .create()
        .unwrap();

    assert!(schema.has_table("public.renamed"));
    assert!(!schema.has_table("public.foo"));
}

#[test]
fn testRenameTableByUnquotedName() {
    let schema = Schema::editor()
        .add_table(create_table("foo", Some("public")))
        .rename_table_by_unquoted_name("foo", "renamed", Some("public"))
        .create()
        .unwrap();

    assert!(schema.has_table("public.renamed"));
    assert!(!schema.has_table("public.foo"));
}

#[test]
fn testDropTable() {
    let schema = Schema::editor()
        .add_table(create_table("foo", None))
        .add_table(create_table("bar", None))
        .drop_table_by_unquoted_name("foo", None)
        .create()
        .unwrap();

    assert!(!schema.has_table("foo"));
    assert!(schema.has_table("bar"));
}

#[test]
fn testDropTableOnAbsentNameThrows() {
    let editor = Schema::editor();

    invalid_modification(editor.drop_table_by_unquoted_name("missing", None));
}

#[test]
fn testDropSequence() {
    let foo = sequence("foo");
    let bar = sequence("bar");

    let schema = Schema::editor()
        .add_sequence(foo)
        .add_sequence(bar)
        .drop_sequence_by_unquoted_name("foo", None)
        .create()
        .unwrap();

    assert!(!schema.has_sequence("foo"));
    assert!(schema.has_sequence("bar"));
}

#[test]
fn testDropSequenceOnAbsentNameThrows() {
    let editor = Schema::editor();

    invalid_modification(editor.drop_sequence_by_unquoted_name("missing", None));
}

#[test]
fn testDefaultNamespaceLookupParity() {
    let editor = Schema::editor()
        .set_default_namespace(Some("public"))
        .add_table(create_table("users", None));

    // Lookup by unqualified name finds it.
    let editor = editor.modify_table_by_unquoted_name(
        "users",
        |te: TableEditor| te.set_comment("via unqualified"),
        None,
    );

    // Lookup by qualified name (resolved against default namespace) finds the same entry.
    let editor = editor.modify_table(
        &OptionallyQualifiedName::unquoted("users", Some("public")).unwrap(),
        |te: TableEditor| te.set_comment("via qualified"),
    );

    let schema = editor.create().unwrap();

    assert_eq!(1, schema.get_tables().len());
}

#[test]
fn testSequenceDefaultNamespaceLookupParity() {
    let editor = Schema::editor()
        .set_default_namespace(Some("public"))
        .add_sequence(sequence("s"));

    // Drop by the qualified form (resolved against the default namespace) finds the entry added unqualified.
    let editor =
        editor.drop_sequence(&OptionallyQualifiedName::unquoted("s", Some("public")).unwrap());

    assert_eq!(
        editor.create().unwrap().get_sequences(),
        [] as [&php2rust_doctrine_dbal::schema::Sequence; 0]
    );
}

#[test]
fn testQualifiedAndUnqualifiedCollideUnderDefaultNamespace() {
    let editor = Schema::editor()
        .set_default_namespace(Some("public"))
        .add_table(create_table("foo", None));

    invalid_modification(editor.add_table(create_table("foo", Some("public"))));
}

#[test]
fn testMixingQualifiedAndUnqualifiedThrows() {
    let editor = Schema::editor()
        .add_table(create_table("a", None))
        .add_table(create_table("b", Some("public")));

    expect_exception("ImproperlyQualifiedName", editor.create());
}

#[test]
fn testMixingUnqualifiedAfterQualifiedThrows() {
    let editor = Schema::editor()
        .add_table(create_table("a", Some("public")))
        .add_table(create_table("b", None));

    expect_exception("ImproperlyQualifiedName", editor.create());
}

#[test]
fn testMixedQualificationUnderDefaultNamespaceIsAccepted() {
    let schema = Schema::editor()
        .set_default_namespace(Some("public"))
        .add_table(create_table("foo", None))
        .add_table(create_table("bar", Some("public")))
        .create()
        .unwrap();

    assert_eq!(2, schema.get_tables().len());
    assert!(schema.has_table("foo"));
    assert!(schema.has_table("public.bar"));
}

#[test]
fn testEditRoundTripPreservesMixedQualificationUnderDefaultNamespace() {
    let schema = Schema::editor()
        .set_default_namespace(Some("public"))
        .set_tables(vec![
            create_table("foo", None),
            create_table("bar", Some("public")),
        ])
        .create()
        .unwrap();

    let result = schema.edit().create().unwrap();

    assert_eq!(2, result.get_tables().len());
    assert!(result.has_table("foo"));
    assert!(result.has_table("public.bar"));
}

#[test]
fn testSetDefaultNamespaceAfterAddingTableThrows() {
    let editor = Schema::editor().add_table(create_table("foo", None));

    invalid_modification(editor.set_default_namespace(Some("public")));
}

#[test]
fn testSetDefaultNamespaceAfterAddingSequenceThrows() {
    let editor = Schema::editor().add_sequence(sequence("a_seq"));

    invalid_modification(editor.set_default_namespace(Some("public")));
}

#[test]
fn testWorkedExampleAddingUniqueIndex() {
    let table_editor = Table::editor()
        .set_unquoted_name("users", None)
        .set_columns(vec![
            Column::editor()
                .set_unquoted_name("id")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
            Column::editor()
                .set_unquoted_name("email")
                .set_type_name(Types::STRING)
                .create()
                .unwrap(),
        ])
        .add_index(
            Index::editor()
                .set_unquoted_name("uniq_users_email")
                .set_type(IndexType::Unique)
                .set_unquoted_column_names(&["email"])
                .create()
                .unwrap(),
        );

    let schema = Schema::editor()
        .add_table(table_editor.create().unwrap())
        .create()
        .unwrap();

    let indexes = schema.get_table("users").unwrap().get_indexes();
    assert_eq!(1, indexes.len());

    let index = indexes.values().next().unwrap();
    assert_eq!(IndexType::Unique, index.get_type().unwrap());
}
