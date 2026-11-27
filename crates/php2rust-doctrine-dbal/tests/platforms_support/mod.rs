//! Port of tests/Platforms/AbstractPlatformTestCase.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, for the platform tests that
//! extend it (`MySQLPlatformTest`, `PostgreSQLPlatformTest`, ...).
//!
//! PHPUnit's abstract test case is the trait [`AbstractPlatformTestCase`]:
//! its abstract and overridable expectation methods (`getGenerateTableSql()`,
//! ...) are the trait's methods, and each inherited test is a generic
//! function here that a test file calls from a `#[test]` of the same
//! name. A subclass's overridden test is written in its own file. The data
//! providers' rows are the recorded ones of the concrete class
//! (tests/data/php.json), so a subclass's provider overrides apply.
//!
//! Upstream: doctrine/dbal tests/Platforms/AbstractPlatformTestCase.php
#![allow(
    dead_code,
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, php_array};
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::deprecations;
use php2rust_doctrine_dbal::platforms::AbstractPlatform;
use php2rust_doctrine_dbal::schema::name::{Identifier, OptionallyQualifiedName, UnqualifiedName};
use php2rust_doctrine_dbal::schema::{
    Column, ColumnDiff, ColumnEditor, CurrentDate, CurrentTimestamp, ForeignKeyConstraint, Index,
    IndexType, PrimaryKeyConstraint, Table, TableDiff, TableDiffParts, TypeObject,
    UniqueConstraint,
};
use php2rust_doctrine_dbal::types::{self, Type, TypeArgument, Types};

use crate::common::{array, provider, text};

/// The expectations of `AbstractPlatformTestCase` that a platform's test
/// gives (the abstract ones) or may override (those with a default).
pub trait AbstractPlatformTestCase {
    type Platform: AbstractPlatform;

    /// The recorded providers' class (`Platforms/MySQLPlatformTest`).
    const PROVIDERS: &'static str;

    /// `createPlatform()`.
    fn create_platform() -> Self::Platform;

    fn get_generate_table_sql() -> &'static str;

    fn get_generate_table_with_multi_column_unique_index_sql() -> &'static [&'static str];

    fn get_generate_index_sql() -> &'static str;

    fn get_generate_unique_index_sql() -> &'static str;

    fn get_generate_foreign_key_sql() -> &'static str;

    fn get_bit_and_comparison_expression_sql(value1: &str, value2: &str) -> String {
        format!("({value1} & {value2})")
    }

    fn get_bit_or_comparison_expression_sql(value1: &str, value2: &str) -> String {
        format!("({value1} | {value2})")
    }

    fn get_quoted_column_in_primary_key_sql() -> &'static [&'static str];

    fn get_quoted_column_in_index_sql() -> &'static [&'static str];

    fn get_quoted_name_in_index_sql() -> &'static [&'static str];

    fn get_quoted_column_in_foreign_key_sql() -> &'static [&'static str];

    fn get_quotes_reserved_keyword_in_unique_constraint_declaration_sql() -> &'static str;

    fn get_quotes_reserved_keyword_in_truncate_table_sql() -> &'static str;

    fn get_quotes_reserved_keyword_in_index_declaration_sql() -> &'static str;

    fn supports_inline_index_declaration() -> bool {
        true
    }

    fn supports_comment_on_statement() -> bool {
        false
    }

    fn get_expected_fixed_length_string_type_declaration_sql_no_length() -> &'static str {
        "CHAR"
    }

    fn get_expected_fixed_length_string_type_declaration_sql_with_length() -> &'static str {
        "CHAR(16)"
    }

    fn get_expected_variable_length_string_type_declaration_sql_no_length() -> &'static str {
        "VARCHAR"
    }

    fn get_expected_variable_length_string_type_declaration_sql_with_length() -> &'static str {
        "VARCHAR(16)"
    }

    fn get_expected_fixed_length_binary_type_declaration_sql_no_length() -> &'static str {
        "BINARY"
    }

    fn get_expected_fixed_length_binary_type_declaration_sql_with_length() -> &'static str {
        "BINARY(16)"
    }

    fn get_expected_variable_length_binary_type_declaration_sql_no_length() -> &'static str {
        "VARBINARY"
    }

    fn get_expected_variable_length_binary_type_declaration_sql_with_length() -> &'static str {
        "VARBINARY(16)"
    }

    fn get_alter_table_rename_index_sql() -> &'static [&'static str] {
        &["DROP INDEX idx_foo", "CREATE INDEX idx_bar ON mytable (id)"]
    }

    fn get_quoted_alter_table_rename_index_sql() -> &'static [&'static str] {
        &[
            "DROP INDEX \"create\"",
            "CREATE INDEX \"select\" ON \"table\" (id)",
            "DROP INDEX \"foo\"",
            "CREATE INDEX \"bar\" ON \"table\" (id)",
        ]
    }

    fn get_alter_table_rename_index_in_schema_sql() -> &'static [&'static str] {
        &[
            "DROP INDEX idx_foo",
            "CREATE INDEX idx_bar ON myschema.mytable (id)",
        ]
    }

    fn get_quoted_alter_table_rename_index_in_schema_sql() -> &'static [&'static str] {
        &[
            "DROP INDEX \"schema\".\"create\"",
            "CREATE INDEX \"select\" ON \"schema\".\"table\" (id)",
            "DROP INDEX \"schema\".\"foo\"",
            "CREATE INDEX \"bar\" ON \"schema\".\"table\" (id)",
        ]
    }

    fn get_quoted_comment_on_column_sql_without_quote_character() -> &'static str {
        "COMMENT ON COLUMN mytable.id IS 'This is a comment'"
    }

    fn get_quoted_comment_on_column_sql_with_quote_character() -> &'static str {
        "COMMENT ON COLUMN mytable.id IS 'It''s a quote !'"
    }

    fn get_comment_on_column_sql() -> &'static [&'static str];

    fn get_alter_string_to_fixed_string_sql() -> &'static [&'static str];

    fn get_generates_alter_table_rename_index_used_by_foreign_key_sql() -> &'static [&'static str];
}

// Helpers.

pub fn sql(text: &str) -> PhpString {
    PhpString::from(text)
}

pub fn strings(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|s| (*s).to_owned()).collect()
}

/// `Column::editor()->setUnquotedName($name)->setTypeName($type)`.
pub fn column(name: &str, type_name: &str) -> ColumnEditor {
    Column::editor()
        .set_unquoted_name(name)
        .set_type_name(type_name)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the argument of set_primary_key_constraint()"
)]
pub fn pk(columns: &[&str]) -> Option<PrimaryKeyConstraint> {
    Some(
        PrimaryKeyConstraint::editor()
            .set_unquoted_column_names(columns)
            .create()
            .unwrap(),
    )
}

/// `new Index($name, $columns, $isUnique, $isPrimary)`.
pub fn index(name: &str, columns: &[&str], is_unique: bool, is_primary: bool) -> Index {
    Index::new(
        Some(name),
        columns.iter().copied(),
        is_unique,
        is_primary,
        Vec::<String>::new(),
        PhpArray::new(),
    )
    .unwrap()
}

/// `$this->expectDeprecationWithIdentifier($link)` around `f`.
pub fn expect_deprecation<T>(link: &str, f: impl FnOnce() -> T) -> T {
    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(link);
    let result = f();
    assert!(
        deprecations::count(link) > before,
        "Expected deprecation with identifier \"{link}\" was not triggered"
    );
    deprecations::disable();
    result
}

/// `$this->expectNoDeprecationWithIdentifier($link)` around `f`.
pub fn expect_no_deprecation<T>(link: &str, f: impl FnOnce() -> T) -> T {
    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(link);
    let result = f();
    assert_eq!(
        deprecations::count(link),
        before,
        "Unexpected deprecation with identifier \"{link}\" was triggered"
    );
    deprecations::disable();
    result
}

/// The table `mytable` (or `$name`) with an `id` primary key.
pub fn mytable_with_id(name: &str, qualifier: Option<&str>) -> Table {
    Table::editor()
        .set_unquoted_name(name, qualifier)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(pk(&["id"]))
        .create()
        .unwrap()
}

/// `renamedIndexes: [$old => new Index($new, ['id'])]`.
pub fn renamed(pairs: &[(&str, &str)]) -> TableDiffParts {
    TableDiffParts {
        renamed_indexes: pairs
            .iter()
            .map(|(old, new)| ((*old).to_owned(), index(new, &["id"], false, false)))
            .collect(),
        ..TableDiffParts::default()
    }
}

/// `new TableDiff($table, changedColumns: [$name => new ColumnDiff($table->getColumn($name), $new)])`.
pub fn changed_column(table: &Table, name: &str, new_column: Column) -> TableDiff {
    TableDiff::new(
        table.clone(),
        TableDiffParts {
            changed_columns: vec![ColumnDiff::new(
                table.get_column(name).unwrap().clone(),
                new_column,
            )],
            ..TableDiffParts::default()
        },
    )
}

/// The rows of one of the concrete class's providers.
pub fn rows<T: AbstractPlatformTestCase>(test: &str) -> Vec<(String, Vec<serde_json::Value>)> {
    provider(T::PROVIDERS, test)
}

// The tests.

pub fn testQuoteIdentifier<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform().quote_identifier("test.test"),
        "\"test\".\"test\""
    );
}

pub fn testReturnsForeignKeyReferentialActionSQL<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testReturnsForeignKeyReferentialActionSQL") {
        assert_eq!(
            T::create_platform()
                .get_foreign_key_referential_action_sql(&text(&args[0]))
                .unwrap(),
            text(&args[1]),
            "{name}"
        );
    }
}

pub fn testGetInvalidForeignKeyReferentialActionSQL<T: AbstractPlatformTestCase>() {
    let e = T::create_platform()
        .get_foreign_key_referential_action_sql("unknown")
        .unwrap_err();
    assert!(e.is_instance_of("InvalidArgumentException"), "{e}");
}

pub fn testGetUnknownDoctrineMappingType<T: AbstractPlatformTestCase>() {
    let e = T::create_platform()
        .get_doctrine_type_mapping("foobar")
        .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

pub fn testRegisterDoctrineMappingType<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    platform
        .register_doctrine_type_mapping("foo", Types::INTEGER)
        .unwrap();
    assert_eq!(
        platform.get_doctrine_type_mapping("foo").unwrap(),
        Types::INTEGER
    );
}

#[derive(Debug)]
struct TestType;

impl Type for TestType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\Type@anonymous"
    }

    fn get_mapped_database_types(&self, _platform: &dyn AbstractPlatform) -> Vec<PhpString> {
        vec![sql("TESTTYPE")]
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_decimal_type_declaration_sql(column)
    }
}

pub fn testCaseInsensitiveDoctrineTypeMappingFromType<T: AbstractPlatformTestCase>() {
    let type_: Arc<dyn Type> = Arc::new(TestType);
    if types::has_type("testtype") {
        types::override_type("testtype", TypeArgument::Instance(type_)).unwrap();
    } else {
        types::add_type("testtype", TypeArgument::Instance(type_)).unwrap();
    }
    assert_eq!(
        T::create_platform()
            .get_doctrine_type_mapping("TeStTyPe")
            .unwrap(),
        "testtype"
    );
}

pub fn testRegisterUnknownDoctrineMappingType<T: AbstractPlatformTestCase>() {
    let e = T::create_platform()
        .register_doctrine_type_mapping("foo", "bar")
        .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

pub fn testCreateWithNoColumns<T: AbstractPlatformTestCase>() {
    let table = Table::new("test").unwrap();
    let e = T::create_platform()
        .get_create_table_sql(&table)
        .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

pub fn testGeneratesTableCreationSql<T: AbstractPlatformTestCase>() {
    let table = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
            column("test", Types::STRING)
                .set_length(Some(255))
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(pk(&["id"]))
        .create()
        .unwrap();

    let sql = T::create_platform().get_create_table_sql(&table).unwrap();
    assert_eq!(sql[0], T::get_generate_table_sql());
}

pub fn testGenerateTableWithMultiColumnUniqueIndex<T: AbstractPlatformTestCase>() {
    let table = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            column("foo", Types::STRING)
                .set_length(Some(255))
                .set_not_null(false)
                .create()
                .unwrap(),
            column("bar", Types::STRING)
                .set_length(Some(255))
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .add_index(
            Index::editor()
                .set_type(IndexType::Unique)
                .set_unquoted_column_names(&["foo", "bar"]),
        )
        .create()
        .unwrap();

    assert_eq!(
        T::create_platform().get_create_table_sql(&table).unwrap(),
        strings(T::get_generate_table_with_multi_column_unique_index_sql())
    );
}

pub fn testGeneratesIndexCreationSql<T: AbstractPlatformTestCase>() {
    let index_def = index("my_idx", &["user_name", "last_login"], false, false);
    assert_eq!(
        T::create_platform()
            .get_create_index_sql(&index_def, "mytable")
            .unwrap(),
        T::get_generate_index_sql()
    );
}

pub fn testGeneratesUniqueIndexCreationSql<T: AbstractPlatformTestCase>() {
    let index_def = index("index_name", &["test", "test2"], true, false);
    assert_eq!(
        T::create_platform()
            .get_create_index_sql(&index_def, "test")
            .unwrap(),
        T::get_generate_unique_index_sql()
    );
}

pub fn testGeneratesPartialIndexesSqlOnlyWhenSupportingPartialIndexes<
    T: AbstractPlatformTestCase,
>() {
    let platform = T::create_platform();
    let where_ = "test IS NULL AND test2 IS NOT NULL";
    let index_def = Index::new(
        Some("name"),
        ["test", "test2"],
        false,
        false,
        Vec::<String>::new(),
        php_array!["where" => where_],
    )
    .unwrap();
    let unique_constraint = UniqueConstraint::editor()
        .set_unquoted_name("name")
        .set_unquoted_column_names(&["test", "test2"])
        .create()
        .unwrap();

    let expected = format!(" WHERE {where_}");

    let mut indexes = Vec::new();

    if T::supports_inline_index_declaration() {
        indexes.push(platform.get_index_declaration_sql(&index_def).unwrap());
    }

    let unique_constraint_sql = platform
        .get_unique_constraint_declaration_sql(&unique_constraint)
        .unwrap();
    assert!(
        !unique_constraint_sql.ends_with(&expected),
        "WHERE clause should NOT be present"
    );

    indexes.push(platform.get_create_index_sql(&index_def, "table").unwrap());

    for index in indexes {
        if platform.supports_partial_indexes() {
            assert!(index.ends_with(&expected), "WHERE clause should be present");
        } else {
            assert!(
                !index.ends_with(&expected),
                "WHERE clause should NOT be present"
            );
        }
    }
}

pub fn testGeneratesForeignKeyCreationSql<T: AbstractPlatformTestCase>() {
    let fk = ForeignKeyConstraint::new(["fk_name_id"], "other_table", ["id"], "", PhpArray::new())
        .unwrap();
    assert_eq!(
        T::create_platform()
            .get_create_foreign_key_sql(&fk, "test")
            .unwrap(),
        T::get_generate_foreign_key_sql()
    );
}

pub fn testGeneratesBitAndComparisonExpressionSql<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform().get_bit_and_comparison_expression("2", "4"),
        T::get_bit_and_comparison_expression_sql("2", "4")
    );
}

pub fn testGeneratesBitOrComparisonExpressionSql<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform().get_bit_or_comparison_expression("2", "4"),
        T::get_bit_or_comparison_expression_sql("2", "4")
    );
}

pub fn testGetCustomColumnDeclarationSql<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_column_declaration_sql(
                "foo",
                &php_array!["columnDefinition" => "MEDIUMINT(6) UNSIGNED"]
            )
            .unwrap(),
        "foo MEDIUMINT(6) UNSIGNED"
    );
}

pub fn testGetDefaultValueDeclarationSQL<T: AbstractPlatformTestCase>() {
    // non-timestamp value will get single quotes
    assert_eq!(
        T::create_platform()
            .get_default_value_declaration_sql(&php_array![
                "typeName" => Types::STRING,
                "default" => "non_timestamp",
            ])
            .unwrap(),
        " DEFAULT 'non_timestamp'"
    );
}

pub fn testGetDefaultValueDeclarationSQLDateTime<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    // timestamps on datetime types should not be quoted
    for type_ in [
        Types::DATETIME_MUTABLE,
        Types::DATETIMETZ_MUTABLE,
        Types::DATETIME_IMMUTABLE,
        Types::DATETIMETZ_IMMUTABLE,
    ] {
        let mut column = php_array!["typeName" => type_];
        column.insert("default", CurrentTimestamp::value());
        assert_eq!(
            platform.get_default_value_declaration_sql(&column).unwrap(),
            format!(" DEFAULT {}", platform.get_current_timestamp_sql())
        );
    }
}

pub fn testGetDefaultValueDeclarationSQLForIntegerTypes<T: AbstractPlatformTestCase>() {
    for type_ in [Types::BIGINT, Types::INTEGER, Types::SMALLINT] {
        assert_eq!(
            T::create_platform()
                .get_default_value_declaration_sql(&php_array!["typeName" => type_, "default" => 1])
                .unwrap(),
            " DEFAULT 1"
        );
    }
}

pub fn testGetDefaultValueDeclarationSQLForDateType<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    let current_date_sql = platform.get_current_date_sql();
    for type_ in [Types::DATE_MUTABLE, Types::DATE_IMMUTABLE] {
        let mut column = php_array!["typeName" => type_];
        column.insert("default", CurrentDate::value());
        assert_eq!(
            platform.get_default_value_declaration_sql(&column).unwrap(),
            format!(" DEFAULT {current_date_sql}")
        );
    }
}

pub fn testGetDefaultValueDeclarationSQLWithLegacyTypeKey<T: AbstractPlatformTestCase>() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        let mut column = PhpArray::new();
        column.insert(
            "type",
            TypeObject::value(types::get_type(Types::INTEGER).unwrap()),
        );
        column.insert("default", Value::Int(1));
        assert_eq!(
            T::create_platform()
                .get_default_value_declaration_sql(&column)
                .unwrap(),
            " DEFAULT 1"
        );
    });
}

pub fn testGetColumnDeclarationListSQLWithLegacyTypeKey<T: AbstractPlatformTestCase>() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        let platform = T::create_platform();
        let expected = platform
            .get_column_declaration_list_sql(&[php_array![
                "name" => "id",
                "notnull" => true,
                "typeName" => Types::INTEGER,
            ]])
            .unwrap();
        let mut column = php_array!["name" => "id", "notnull" => true];
        column.insert(
            "type",
            TypeObject::value(types::get_type(Types::INTEGER).unwrap()),
        );
        assert_eq!(
            platform.get_column_declaration_list_sql(&[column]).unwrap(),
            expected
        );
    });
}

pub fn testGetColumnDeclarationSQLWithTypeNameDoesNotTriggerDeprecation<
    T: AbstractPlatformTestCase,
>() {
    expect_no_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        T::create_platform()
            .get_column_declaration_sql(
                "id",
                &php_array!["name" => "id", "typeName" => Types::INTEGER],
            )
            .unwrap();
    });
}

pub fn testGetColumnDeclarationSQLWithoutType<T: AbstractPlatformTestCase>() {
    let e = T::create_platform()
        .get_column_declaration_sql("id", &php_array!["name" => "id"])
        .unwrap_err();
    assert!(
        e.is_instance_of(Exception::INVALID_COLUMN_DECLARATION),
        "{e}"
    );
    assert!(
        e.message()
            .to_string_lossy()
            .contains("Column \"id\" has no type."),
        "{e}"
    );
}

pub fn testKeywordList<T: AbstractPlatformTestCase>() {
    let keyword_list = T::create_platform().get_reserved_keywords_list();
    assert!(keyword_list.is_keyword("table"));
}

pub fn testQuotedColumnInPrimaryKeyPropagation<T: AbstractPlatformTestCase>() {
    let table = Table::editor()
        .set_quoted_name("quoted", None)
        .set_columns(vec![
            column("create", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(pk(&["create"]))
        .create()
        .unwrap();

    assert_eq!(
        T::create_platform().get_create_table_sql(&table).unwrap(),
        strings(T::get_quoted_column_in_primary_key_sql())
    );
}

pub fn testQuotedColumnInIndexPropagation<T: AbstractPlatformTestCase>() {
    let table = Table::editor()
        .set_quoted_name("quoted", None)
        .set_columns(vec![
            column("create", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
        ])
        .add_index(Index::editor().set_unquoted_column_names(&["create"]))
        .create()
        .unwrap();

    assert_eq!(
        T::create_platform().get_create_table_sql(&table).unwrap(),
        strings(T::get_quoted_column_in_index_sql())
    );
}

pub fn testQuotedNameInIndexSQL<T: AbstractPlatformTestCase>() {
    let table = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            column("column1", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
        ])
        .set_indexes(vec![
            Index::editor()
                .set_quoted_name("key")
                .set_unquoted_column_names(&["column1"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        T::create_platform().get_create_table_sql(&table).unwrap(),
        strings(T::get_quoted_name_in_index_sql())
    );
}

pub fn testQuotedColumnInForeignKeyPropagation<T: AbstractPlatformTestCase>() {
    let referencing_column_names = vec![
        UnqualifiedName::unquoted("create").unwrap(),
        UnqualifiedName::unquoted("foo").unwrap(),
        UnqualifiedName::quoted("bar").unwrap(),
    ];
    let referenced_column_names = vec![
        UnqualifiedName::unquoted("create").unwrap(),
        UnqualifiedName::unquoted("bar").unwrap(),
        UnqualifiedName::quoted("foo-bar").unwrap(),
    ];

    let string_column = |editor: ColumnEditor| {
        editor
            .set_type_name(Types::STRING)
            .set_length(Some(255))
            .create()
            .unwrap()
    };

    let table = Table::editor()
        .set_quoted_name("quoted", None)
        .set_columns(vec![
            string_column(Column::editor().set_unquoted_name("create")),
            string_column(Column::editor().set_unquoted_name("foo")),
            string_column(Column::editor().set_quoted_name("bar")),
        ])
        .set_foreign_key_constraints(vec![
            ForeignKeyConstraint::editor()
                .set_unquoted_name("FK_WITH_RESERVED_KEYWORD")
                .set_referencing_column_names(referencing_column_names.clone())
                .set_unquoted_referenced_table_name("foreign", None)
                .set_referenced_column_names(referenced_column_names.clone())
                .create()
                .unwrap(),
            ForeignKeyConstraint::editor()
                .set_unquoted_name("FK_WITH_NON_RESERVED_KEYWORD")
                .set_referencing_column_names(referencing_column_names.clone())
                .set_unquoted_referenced_table_name("foo", None)
                .set_referenced_column_names(referenced_column_names.clone())
                .create()
                .unwrap(),
            ForeignKeyConstraint::editor()
                .set_unquoted_name("FK_WITH_INTENDED_QUOTATION")
                .set_referencing_column_names(referencing_column_names)
                .set_quoted_referenced_table_name("foo-bar", None)
                .set_referenced_column_names(referenced_column_names)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        T::create_platform().get_create_table_sql(&table).unwrap(),
        strings(T::get_quoted_column_in_foreign_key_sql())
    );
}

pub fn testQuotesReservedKeywordInUniqueConstraintDeclarationSQL<T: AbstractPlatformTestCase>() {
    let constraint =
        UniqueConstraint::new("select", ["foo"], Vec::<String>::new(), PhpArray::new());
    assert_eq!(
        T::create_platform()
            .get_unique_constraint_declaration_sql(&constraint)
            .unwrap(),
        T::get_quotes_reserved_keyword_in_unique_constraint_declaration_sql()
    );
}

pub fn testQuotesReservedKeywordInTruncateTableSQL<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform().get_truncate_table_sql("select", false),
        T::get_quotes_reserved_keyword_in_truncate_table_sql()
    );
}

pub fn testQuotesReservedKeywordInIndexDeclarationSQL<T: AbstractPlatformTestCase>() {
    let index = index("select", &["foo"], false, false);
    let result = T::create_platform().get_index_declaration_sql(&index);

    if !T::supports_inline_index_declaration() {
        let e = result.unwrap_err();
        assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
        return;
    }

    assert_eq!(
        result.unwrap(),
        T::get_quotes_reserved_keyword_in_index_declaration_sql()
    );
}

pub fn testSupportsCommentOnStatement<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform().supports_comment_on_statement(),
        T::supports_comment_on_statement()
    );
}

pub fn testGetCreateSchemaSQL<T: AbstractPlatformTestCase>() {
    let e = T::create_platform()
        .get_create_schema_sql("schema")
        .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

pub fn testAlterTableChangeQuotedColumn<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    let table = Table::editor()
        .set_unquoted_name("mytable", None)
        .set_columns(vec![column("select", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    let table_diff = changed_column(
        &table,
        "select",
        column("select", Types::STRING)
            .set_length(Some(255))
            .create()
            .unwrap(),
    );

    assert!(
        platform
            .get_alter_table_sql(&table_diff)
            .unwrap()
            .join(";")
            .contains(&platform.quote_single_identifier("select"))
    );
}

pub fn testGetFixedLengthStringTypeDeclarationSQLNoLength<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_string_type_declaration_sql(&php_array!["fixed" => true])
            .unwrap(),
        sql(T::get_expected_fixed_length_string_type_declaration_sql_no_length())
    );
}

pub fn testGetFixedLengthStringTypeDeclarationSQLWithLength<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_string_type_declaration_sql(&php_array!["fixed" => true, "length" => 16])
            .unwrap(),
        sql(T::get_expected_fixed_length_string_type_declaration_sql_with_length())
    );
}

pub fn testGetVariableLengthStringTypeDeclarationSQLNoLength<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_string_type_declaration_sql(&php_array!["name" => "email"])
            .unwrap(),
        sql(T::get_expected_variable_length_string_type_declaration_sql_no_length())
    );
}

pub fn testGetVariableLengthStringTypeDeclarationSQLWithLength<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_string_type_declaration_sql(&php_array!["length" => 16])
            .unwrap(),
        sql(T::get_expected_variable_length_string_type_declaration_sql_with_length())
    );
}

pub fn testGetFixedLengthBinaryTypeDeclarationSQLNoLength<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_binary_type_declaration_sql(&php_array!["name" => "checksum", "fixed" => true])
            .unwrap(),
        sql(T::get_expected_fixed_length_binary_type_declaration_sql_no_length())
    );
}

pub fn testGetFixedLengthBinaryTypeDeclarationSQLWithLength<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_binary_type_declaration_sql(&php_array!["fixed" => true, "length" => 16])
            .unwrap(),
        sql(T::get_expected_fixed_length_binary_type_declaration_sql_with_length())
    );
}

pub fn testGetVariableLengthBinaryTypeDeclarationSQLNoLength<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_binary_type_declaration_sql(&php_array!["name" => "attachment"])
            .unwrap(),
        sql(T::get_expected_variable_length_binary_type_declaration_sql_no_length())
    );
}

pub fn testGetVariableLengthBinaryTypeDeclarationSQLWithLength<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_binary_type_declaration_sql(&php_array!["length" => 16])
            .unwrap(),
        sql(T::get_expected_variable_length_binary_type_declaration_sql_with_length())
    );
}

/// `$this->expectException(InvalidColumnDeclaration::class)` around a
/// declaration.
pub fn assert_invalid_column_declaration(result: Result<PhpString, Exception>) {
    let e = result.unwrap_err();
    assert!(
        e.is_instance_of(Exception::INVALID_COLUMN_DECLARATION),
        "{e}"
    );
}

pub fn testGetDecimalTypeDeclarationSQLNoPrecision<T: AbstractPlatformTestCase>() {
    assert_invalid_column_declaration(
        T::create_platform()
            .get_decimal_type_declaration_sql(&php_array!["name" => "price", "scale" => 2]),
    );
}

pub fn testGetDecimalTypeDeclarationSQLNoScale<T: AbstractPlatformTestCase>() {
    assert_invalid_column_declaration(
        T::create_platform()
            .get_decimal_type_declaration_sql(&php_array!["name" => "price", "precision" => 10]),
    );
}

pub fn testReturnsJsonTypeDeclarationSQL<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    let mut column = php_array!["length" => 666, "notnull" => true];
    column.insert(
        "type",
        TypeObject::value(types::get_type(Types::JSON).unwrap()),
    );
    assert_eq!(
        platform.get_json_type_declaration_sql(&column).unwrap(),
        platform.get_clob_type_declaration_sql(&column).unwrap()
    );
}

pub fn testReturnsJsonbTypeDeclarationSQL<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    expect_deprecation("https://github.com/doctrine/dbal/pull/6939", || {
        assert_eq!(
            platform
                .get_json_type_declaration_sql(&php_array!["jsonb" => true])
                .unwrap(),
            platform
                .get_jsonb_type_declaration_sql(&PhpArray::new())
                .unwrap()
        );
    });
}

pub fn testAlterTableRenameIndex<T: AbstractPlatformTestCase>() {
    let table = mytable_with_id("mytable", None);
    let table_diff = TableDiff::new(table, renamed(&[("idx_foo", "idx_bar")]));

    assert_eq!(
        T::create_platform()
            .get_alter_table_sql(&table_diff)
            .unwrap(),
        strings(T::get_alter_table_rename_index_sql())
    );
}

pub fn testQuotesAlterTableRenameIndex<T: AbstractPlatformTestCase>() {
    let table = mytable_with_id("table", None);
    let table_diff = TableDiff::new(table, renamed(&[("create", "select"), ("`foo`", "`bar`")]));

    assert_eq!(
        T::create_platform()
            .get_alter_table_sql(&table_diff)
            .unwrap(),
        strings(T::get_quoted_alter_table_rename_index_sql())
    );
}

pub fn testAlterTableRenameIndexInSchema<T: AbstractPlatformTestCase>() {
    let table = mytable_with_id("mytable", Some("myschema"));
    let table_diff = TableDiff::new(table, renamed(&[("idx_foo", "idx_bar")]));

    assert_eq!(
        T::create_platform()
            .get_alter_table_sql(&table_diff)
            .unwrap(),
        strings(T::get_alter_table_rename_index_in_schema_sql())
    );
}

pub fn testQuotesAlterTableRenameIndexInSchema<T: AbstractPlatformTestCase>() {
    let table = Table::editor()
        .set_name(OptionallyQualifiedName::new(
            Identifier::unquoted("table").unwrap(),
            Some(Identifier::quoted("schema").unwrap()),
        ))
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(pk(&["id"]))
        .create()
        .unwrap();

    let table_diff = TableDiff::new(table, renamed(&[("create", "select"), ("`foo`", "`bar`")]));

    assert_eq!(
        T::create_platform()
            .get_alter_table_sql(&table_diff)
            .unwrap(),
        strings(T::get_quoted_alter_table_rename_index_in_schema_sql())
    );
}

pub fn testGetCommentOnColumnSQLWithoutQuoteCharacter<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform().get_comment_on_column_sql("mytable", "id", "This is a comment"),
        T::get_quoted_comment_on_column_sql_without_quote_character()
    );
}

pub fn testGetCommentOnColumnSQLWithQuoteCharacter<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform().get_comment_on_column_sql("mytable", "id", "It's a quote !"),
        T::get_quoted_comment_on_column_sql_with_quote_character()
    );
}

pub fn testGetCommentOnColumnSQL<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    assert_eq!(
        strings(T::get_comment_on_column_sql()),
        vec![
            // regular identifiers
            platform.get_comment_on_column_sql("foo", "bar", "comment"),
            // explicitly quoted identifiers
            platform.get_comment_on_column_sql("`Foo`", "`BAR`", "comment"),
            // reserved keyword identifiers
            platform.get_comment_on_column_sql("select", "from", "comment"),
        ]
    );
}

pub fn testGeneratesInlineColumnCommentSQL<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    if !platform.supports_inline_column_comments() {
        // markTestSkipped: the platform does not support inline column comments.
        return;
    }
    for (name, args) in rows::<T>("testGeneratesInlineColumnCommentSQL") {
        assert_eq!(
            platform
                .get_inline_column_comment_sql(&text(&args[0]))
                .unwrap(),
            text(&args[1]),
            "{name}"
        );
    }
}

pub fn testThrowsExceptionOnGeneratingInlineColumnCommentSQLIfUnsupported<
    T: AbstractPlatformTestCase,
>() {
    let platform = T::create_platform();
    if platform.supports_inline_column_comments() {
        // markTestSkipped: the platform supports inline column comments.
        return;
    }

    let e = platform
        .get_inline_column_comment_sql("unsupported")
        .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
    assert_eq!(
        e.message().to_string_lossy(),
        "Operation \"Doctrine\\DBAL\\Platforms\\AbstractPlatform::getInlineColumnCommentSQL\" is not supported by platform."
    );
    assert_eq!(e.code(), 0);
}

pub fn testQuoteStringLiteral<T: AbstractPlatformTestCase>() {
    let platform = T::create_platform();
    assert_eq!(platform.quote_string_literal("No quote"), "'No quote'");
    assert_eq!(
        platform.quote_string_literal("It's a quote"),
        "'It''s a quote'"
    );
    assert_eq!(platform.quote_string_literal("'"), "''''");
}

pub fn testReturnsGuidTypeDeclarationSQL<T: AbstractPlatformTestCase>() {
    let e = T::create_platform()
        .get_guid_type_declaration_sql(&PhpArray::new())
        .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

pub fn testAlterStringToFixedString<T: AbstractPlatformTestCase>() {
    let table = Table::editor()
        .set_unquoted_name("mytable", None)
        .set_columns(vec![
            column("name", Types::STRING)
                .set_length(Some(2))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let table_diff = changed_column(
        &table,
        "name",
        column("name", Types::STRING)
            .set_fixed(true)
            .set_length(Some(2))
            .create()
            .unwrap(),
    );

    assert_eq!(
        T::create_platform()
            .get_alter_table_sql(&table_diff)
            .unwrap(),
        strings(T::get_alter_string_to_fixed_string_sql())
    );
}

pub fn testGeneratesAlterTableRenameIndexUsedByForeignKeySQL<T: AbstractPlatformTestCase>() {
    let fk = |name: &str, column: &str| {
        ForeignKeyConstraint::editor()
            .set_unquoted_name(name)
            .set_unquoted_referencing_column_names(&[column])
            .set_unquoted_referenced_table_name("foreign_table", None)
            .set_unquoted_referenced_column_names(&["id"])
            .create()
            .unwrap()
    };
    let idx = |name: &str, column: &str| {
        Index::editor()
            .set_unquoted_name(name)
            .set_unquoted_column_names(&[column])
            .create()
            .unwrap()
    };
    let primary_table = Table::editor()
        .set_unquoted_name("mytable", None)
        .set_columns(vec![
            column("foo", Types::INTEGER).create().unwrap(),
            column("bar", Types::INTEGER).create().unwrap(),
            column("baz", Types::INTEGER).create().unwrap(),
        ])
        .set_indexes(vec![idx("idx_foo", "foo"), idx("idx_bar", "bar")])
        .set_foreign_key_constraints(vec![fk("fk_foo", "foo"), fk("fk_bar", "bar")])
        .create()
        .unwrap();

    let table_diff = TableDiff::new(
        primary_table,
        TableDiffParts {
            renamed_indexes: vec![(
                "idx_foo".to_owned(),
                index("idx_foo_renamed", &["foo"], false, false),
            )],
            ..TableDiffParts::default()
        },
    );

    assert_eq!(
        T::create_platform()
            .get_alter_table_sql(&table_diff)
            .unwrap(),
        strings(T::get_generates_alter_table_rename_index_used_by_foreign_key_sql())
    );
}

pub fn testGeneratesDecimalTypeDeclarationSQL<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testGeneratesDecimalTypeDeclarationSQL") {
        assert_eq!(
            T::create_platform()
                .get_decimal_type_declaration_sql(&array(&args[0]))
                .unwrap(),
            sql(&text(&args[1])),
            "{name}"
        );
    }
}

pub fn testGeneratesFloatDeclarationSQL<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testGeneratesFloatDeclarationSQL") {
        assert_eq!(
            T::create_platform()
                .get_float_declaration_sql(&array(&args[0]))
                .unwrap(),
            sql(&text(&args[1])),
            "{name}"
        );
    }
}

pub fn testGeneratesSmallFloatDeclarationSQL<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testGeneratesSmallFloatDeclarationSQL") {
        assert_eq!(
            T::create_platform()
                .get_small_float_declaration_sql(&array(&args[0]))
                .unwrap(),
            sql(&text(&args[1])),
            "{name}"
        );
    }
}

pub fn testItEscapesStringsForLike<T: AbstractPlatformTestCase>() {
    assert_eq!(
        T::create_platform().escape_string_for_like("_25% off_ your next purchase \\o/", "\\"),
        "\\_25\\% off\\_ your next purchase \\\\o/"
    );
}

pub fn testZeroOffsetWithoutLimitIsIgnored<T: AbstractPlatformTestCase>() {
    let query = "SELECT * FROM user";
    assert_eq!(
        T::create_platform()
            .modify_limit_query(query, None, 0)
            .unwrap(),
        query
    );
}

pub fn testAsciiSQLDeclaration<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testAsciiSQLDeclaration") {
        assert_eq!(
            T::create_platform()
                .get_ascii_string_type_declaration_sql(&array(&args[1]))
                .unwrap(),
            sql(&text(&args[0])),
            "{name}"
        );
    }
}

pub fn testGetEnumDeclarationSQL<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testGetEnumDeclarationSQL") {
        let mut column = PhpArray::new();
        column.insert("values", Value::Array(array(&args[0])));
        assert_eq!(
            T::create_platform()
                .get_enum_declaration_sql(&column)
                .unwrap(),
            sql(&text(&args[1])),
            "{name}"
        );
    }
}

pub fn testGetEnumDeclarationWithLengthSQL<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testGetEnumDeclarationWithLengthSQL") {
        let mut column = PhpArray::new();
        column.insert("values", Value::Array(array(&args[0])));
        column.insert("length", php_value::vectors::decode(&args[1]).unwrap());
        assert_eq!(
            T::create_platform()
                .get_enum_declaration_sql(&column)
                .unwrap(),
            sql(&text(&args[2])),
            "{name}"
        );
    }
}

pub fn testGetEnumDeclarationExceptionWithLengthSQL<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testGetEnumDeclarationExceptionWithLengthSQL") {
        let mut column = PhpArray::new();
        column.insert("values", Value::Array(array(&args[0])));
        column.insert("length", php_value::vectors::decode(&args[1]).unwrap());
        let e = T::create_platform()
            .get_enum_declaration_sql(&column)
            .unwrap_err();
        assert!(e.is_instance_of("InvalidArgumentException"), "{name}: {e}");
    }
}

pub fn testGetEnumDeclarationSQLWithInvalidValues<T: AbstractPlatformTestCase>() {
    for (name, args) in rows::<T>("testGetEnumDeclarationSQLWithInvalidValues") {
        let e = T::create_platform()
            .get_enum_declaration_sql(&array(&args[0]))
            .unwrap_err();
        assert!(
            e.is_instance_of(Exception::COLUMN_VALUES_REQUIRED),
            "{name}: {e}"
        );
    }
}
