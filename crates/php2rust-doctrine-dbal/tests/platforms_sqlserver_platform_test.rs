//! Port of tests/Platforms/SQLServerPlatformTest.php with the tests it
//! inherits from tests/Platforms/AbstractPlatformTestCase.php, at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. The test names
//! are PHPUnit's; an inherited test calls the shared port
//! (`tests/platforms_support`) on `Case`, whose impl holds this class's
//! expectations; `SQLServerPlatformTest`'s own and overridden tests are
//! written in place. The data providers' rows are the recorded ones
//! (tests/data/php.json).
//!
//! Upstream: doctrine/dbal tests/Platforms/SQLServerPlatformTest.php
//! Upstream: doctrine/dbal tests/Platforms/AbstractPlatformTestCase.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod platforms_support;

use common::bytes;
use php_value::php_array;
use php2rust_doctrine_dbal::platforms::{AbstractPlatform, SQLServerPlatform};
use php2rust_doctrine_dbal::schema::{
    Column, ColumnDiff, CurrentDate, PrimaryKeyConstraint, Sequence, Table, TableDiff,
    TableDiffParts, TypeObject,
};
use php2rust_doctrine_dbal::types::{self, Types};
use php2rust_doctrine_dbal::{LockMode, TransactionIsolationLevel};
use platforms_support::{
    AbstractPlatformTestCase, assert_invalid_column_declaration, column, index, pk, rows, sql,
};

/// `SQLServerPlatformTest`.
struct Case;

impl AbstractPlatformTestCase for Case {
    type Platform = SQLServerPlatform;

    const PROVIDERS: &'static str = "Platforms/SQLServerPlatformTest";

    fn create_platform() -> SQLServerPlatform {
        SQLServerPlatform::new()
    }

    fn get_generate_table_sql() -> &'static str {
        "CREATE TABLE test (id INT IDENTITY NOT NULL, test NVARCHAR(255), PRIMARY KEY (id))"
    }

    fn get_generate_table_with_multi_column_unique_index_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE test (foo NVARCHAR(255), bar NVARCHAR(255))",
            "CREATE UNIQUE INDEX UNIQ_D87F7E0C8C73652176FF8CAA ON test (foo, bar) \
             WHERE foo IS NOT NULL AND bar IS NOT NULL",
        ]
    }

    fn get_generate_index_sql() -> &'static str {
        "CREATE INDEX my_idx ON mytable (user_name, last_login)"
    }

    fn get_generate_unique_index_sql() -> &'static str {
        "CREATE UNIQUE INDEX index_name ON test (test, test2) WHERE test IS NOT NULL AND test2 IS NOT NULL"
    }

    fn get_generate_foreign_key_sql() -> &'static str {
        "ALTER TABLE test ADD FOREIGN KEY (fk_name_id) REFERENCES other_table (id)"
    }

    fn get_quoted_column_in_primary_key_sql() -> &'static [&'static str] {
        &["CREATE TABLE [quoted] ([create] NVARCHAR(255) NOT NULL, PRIMARY KEY ([create]))"]
    }

    fn get_quoted_column_in_index_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE [quoted] ([create] NVARCHAR(255) NOT NULL)",
            "CREATE INDEX IDX_22660D028FD6E0FB ON [quoted] ([create])",
        ]
    }

    fn get_quoted_name_in_index_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE test (column1 NVARCHAR(255) NOT NULL)",
            "CREATE INDEX [key] ON test (column1)",
        ]
    }

    fn get_quoted_column_in_foreign_key_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE [quoted] ([create] NVARCHAR(255) NOT NULL, \
             foo NVARCHAR(255) NOT NULL, [bar] NVARCHAR(255) NOT NULL)",
            "CREATE INDEX IDX_22660D028FD6E0FB8C7365216D704F76 ON [quoted] ([create], foo, [bar])",
            "ALTER TABLE [quoted] ADD CONSTRAINT FK_WITH_RESERVED_KEYWORD \
             FOREIGN KEY ([create], foo, [bar]) REFERENCES [foreign] ([create], bar, [foo-bar])",
            "ALTER TABLE [quoted] ADD CONSTRAINT FK_WITH_NON_RESERVED_KEYWORD \
             FOREIGN KEY ([create], foo, [bar]) REFERENCES foo ([create], bar, [foo-bar])",
            "ALTER TABLE [quoted] ADD CONSTRAINT FK_WITH_INTENDED_QUOTATION \
             FOREIGN KEY ([create], foo, [bar]) REFERENCES [foo-bar] ([create], bar, [foo-bar])",
        ]
    }

    fn get_expected_fixed_length_string_type_declaration_sql_no_length() -> &'static str {
        "NCHAR"
    }

    fn get_expected_fixed_length_string_type_declaration_sql_with_length() -> &'static str {
        "NCHAR(16)"
    }

    fn get_expected_variable_length_string_type_declaration_sql_with_length() -> &'static str {
        "NVARCHAR(16)"
    }

    fn get_alter_table_rename_index_sql() -> &'static [&'static str] {
        &["EXEC [sp_rename] N'mytable.idx_foo', N'idx_bar', N'INDEX'"]
    }

    fn get_quoted_alter_table_rename_index_sql() -> &'static [&'static str] {
        &[
            "EXEC [sp_rename] N'[table].[create]', N'select', N'INDEX'",
            "EXEC [sp_rename] N'[table].[foo]', N'bar', N'INDEX'",
        ]
    }

    fn get_alter_table_rename_index_in_schema_sql() -> &'static [&'static str] {
        &["EXEC [sp_rename] N'myschema.mytable.idx_foo', N'idx_bar', N'INDEX'"]
    }

    fn get_quoted_alter_table_rename_index_in_schema_sql() -> &'static [&'static str] {
        &[
            "EXEC [sp_rename] N'[schema].[table].[create]', N'select', N'INDEX'",
            "EXEC [sp_rename] N'[schema].[table].[foo]', N'bar', N'INDEX'",
        ]
    }

    fn get_comment_on_column_sql() -> &'static [&'static str] {
        &[
            "COMMENT ON COLUMN foo.bar IS 'comment'",
            "COMMENT ON COLUMN [Foo].[BAR] IS 'comment'",
            "COMMENT ON COLUMN [select].[from] IS 'comment'",
        ]
    }

    fn get_quotes_reserved_keyword_in_unique_constraint_declaration_sql() -> &'static str {
        "CONSTRAINT [select] UNIQUE (foo)"
    }

    fn get_quotes_reserved_keyword_in_index_declaration_sql() -> &'static str {
        "INDEX [select] (foo)"
    }

    fn get_quotes_reserved_keyword_in_truncate_table_sql() -> &'static str {
        "TRUNCATE TABLE [select]"
    }

    fn get_alter_string_to_fixed_string_sql() -> &'static [&'static str] {
        &["ALTER TABLE mytable ALTER COLUMN name NCHAR(2) NOT NULL"]
    }

    fn get_generates_alter_table_rename_index_used_by_foreign_key_sql() -> &'static [&'static str] {
        &["EXEC [sp_rename] N'mytable.idx_foo', N'idx_foo_renamed', N'INDEX'"]
    }
}

fn platform() -> SQLServerPlatform {
    Case::create_platform()
}

/// `$this->platform->modifyLimitQuery($query, $limit, $offset)`.
fn modify_limit_query(query: &str, limit: Option<i64>, offset: i64) -> String {
    platform().modify_limit_query(query, limit, offset).unwrap()
}

fn integer_column(name: &str) -> Column {
    column(name, Types::INTEGER).create().unwrap()
}

/// `Table::editor()->setUnquotedName('mytable', 'testschema')` with an
/// `id` column.
fn schema_mytable() -> Table {
    Table::editor()
        .set_unquoted_name("mytable", Some("testschema"))
        .set_columns(vec![integer_column("id")])
        .create()
        .unwrap()
}

/// `new TableDiff($table, changedColumns: ['quota' => new ColumnDiff($old, $new)])`.
fn changed_quota(old: Column, new: Column) -> TableDiff {
    TableDiff::new(
        schema_mytable(),
        TableDiffParts {
            changed_columns: vec![ColumnDiff::new(old, new)],
            ..TableDiffParts::default()
        },
    )
}

#[test]
fn testAlterStringToFixedString() {
    platforms_support::testAlterStringToFixedString::<Case>();
}

#[test]
fn testAlterTableChangeQuotedColumn() {
    platforms_support::testAlterTableChangeQuotedColumn::<Case>();
}

#[test]
fn testAlterTableRenameIndex() {
    platforms_support::testAlterTableRenameIndex::<Case>();
}

#[test]
fn testAlterTableRenameIndexInSchema() {
    platforms_support::testAlterTableRenameIndexInSchema::<Case>();
}

#[test]
fn testAsciiSQLDeclaration() {
    platforms_support::testAsciiSQLDeclaration::<Case>();
}

#[test]
fn testCaseInsensitiveDoctrineTypeMappingFromType() {
    platforms_support::testCaseInsensitiveDoctrineTypeMappingFromType::<Case>();
}

#[test]
fn testCreateWithNoColumns() {
    platforms_support::testCreateWithNoColumns::<Case>();
}

#[test]
fn testGenerateTableWithMultiColumnUniqueIndex() {
    platforms_support::testGenerateTableWithMultiColumnUniqueIndex::<Case>();
}

#[test]
fn testGeneratesAlterTableRenameIndexUsedByForeignKeySQL() {
    platforms_support::testGeneratesAlterTableRenameIndexUsedByForeignKeySQL::<Case>();
}

#[test]
fn testGeneratesBitAndComparisonExpressionSql() {
    platforms_support::testGeneratesBitAndComparisonExpressionSql::<Case>();
}

#[test]
fn testGeneratesBitOrComparisonExpressionSql() {
    platforms_support::testGeneratesBitOrComparisonExpressionSql::<Case>();
}

#[test]
fn testGeneratesDecimalTypeDeclarationSQL() {
    platforms_support::testGeneratesDecimalTypeDeclarationSQL::<Case>();
}

#[test]
fn testGeneratesFloatDeclarationSQL() {
    platforms_support::testGeneratesFloatDeclarationSQL::<Case>();
}

#[test]
fn testGeneratesForeignKeyCreationSql() {
    platforms_support::testGeneratesForeignKeyCreationSql::<Case>();
}

#[test]
fn testGeneratesIndexCreationSql() {
    platforms_support::testGeneratesIndexCreationSql::<Case>();
}

#[test]
fn testGeneratesInlineColumnCommentSQL() {
    platforms_support::testGeneratesInlineColumnCommentSQL::<Case>();
}

#[test]
fn testGeneratesPartialIndexesSqlOnlyWhenSupportingPartialIndexes() {
    platforms_support::testGeneratesPartialIndexesSqlOnlyWhenSupportingPartialIndexes::<Case>();
}

#[test]
fn testGeneratesSmallFloatDeclarationSQL() {
    platforms_support::testGeneratesSmallFloatDeclarationSQL::<Case>();
}

#[test]
fn testGeneratesTableCreationSql() {
    platforms_support::testGeneratesTableCreationSql::<Case>();
}

#[test]
fn testGeneratesUniqueIndexCreationSql() {
    platforms_support::testGeneratesUniqueIndexCreationSql::<Case>();
}

#[test]
fn testGetColumnDeclarationListSQLWithLegacyTypeKey() {
    platforms_support::testGetColumnDeclarationListSQLWithLegacyTypeKey::<Case>();
}

#[test]
fn testGetColumnDeclarationSQLWithTypeNameDoesNotTriggerDeprecation() {
    platforms_support::testGetColumnDeclarationSQLWithTypeNameDoesNotTriggerDeprecation::<Case>();
}

#[test]
fn testGetColumnDeclarationSQLWithoutType() {
    platforms_support::testGetColumnDeclarationSQLWithoutType::<Case>();
}

#[test]
fn testGetCommentOnColumnSQL() {
    platforms_support::testGetCommentOnColumnSQL::<Case>();
}

#[test]
fn testGetCommentOnColumnSQLWithQuoteCharacter() {
    platforms_support::testGetCommentOnColumnSQLWithQuoteCharacter::<Case>();
}

#[test]
fn testGetCommentOnColumnSQLWithoutQuoteCharacter() {
    platforms_support::testGetCommentOnColumnSQLWithoutQuoteCharacter::<Case>();
}

#[test]
fn testGetCustomColumnDeclarationSql() {
    platforms_support::testGetCustomColumnDeclarationSql::<Case>();
}

#[test]
fn testGetDecimalTypeDeclarationSQLNoPrecision() {
    platforms_support::testGetDecimalTypeDeclarationSQLNoPrecision::<Case>();
}

#[test]
fn testGetDecimalTypeDeclarationSQLNoScale() {
    platforms_support::testGetDecimalTypeDeclarationSQLNoScale::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQL() {
    platforms_support::testGetDefaultValueDeclarationSQL::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQLDateTime() {
    platforms_support::testGetDefaultValueDeclarationSQLDateTime::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQLForIntegerTypes() {
    platforms_support::testGetDefaultValueDeclarationSQLForIntegerTypes::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQLWithLegacyTypeKey() {
    platforms_support::testGetDefaultValueDeclarationSQLWithLegacyTypeKey::<Case>();
}

#[test]
fn testGetEnumDeclarationExceptionWithLengthSQL() {
    platforms_support::testGetEnumDeclarationExceptionWithLengthSQL::<Case>();
}

#[test]
fn testGetEnumDeclarationSQL() {
    platforms_support::testGetEnumDeclarationSQL::<Case>();
}

#[test]
fn testGetEnumDeclarationSQLWithInvalidValues() {
    platforms_support::testGetEnumDeclarationSQLWithInvalidValues::<Case>();
}

#[test]
fn testGetEnumDeclarationWithLengthSQL() {
    platforms_support::testGetEnumDeclarationWithLengthSQL::<Case>();
}

#[test]
fn testGetFixedLengthBinaryTypeDeclarationSQLNoLength() {
    platforms_support::testGetFixedLengthBinaryTypeDeclarationSQLNoLength::<Case>();
}

#[test]
fn testGetFixedLengthBinaryTypeDeclarationSQLWithLength() {
    platforms_support::testGetFixedLengthBinaryTypeDeclarationSQLWithLength::<Case>();
}

#[test]
fn testGetFixedLengthStringTypeDeclarationSQLNoLength() {
    platforms_support::testGetFixedLengthStringTypeDeclarationSQLNoLength::<Case>();
}

#[test]
fn testGetFixedLengthStringTypeDeclarationSQLWithLength() {
    platforms_support::testGetFixedLengthStringTypeDeclarationSQLWithLength::<Case>();
}

#[test]
fn testGetInvalidForeignKeyReferentialActionSQL() {
    platforms_support::testGetInvalidForeignKeyReferentialActionSQL::<Case>();
}

#[test]
fn testGetUnknownDoctrineMappingType() {
    platforms_support::testGetUnknownDoctrineMappingType::<Case>();
}

#[test]
fn testGetVariableLengthBinaryTypeDeclarationSQLWithLength() {
    platforms_support::testGetVariableLengthBinaryTypeDeclarationSQLWithLength::<Case>();
}

#[test]
fn testGetVariableLengthStringTypeDeclarationSQLWithLength() {
    platforms_support::testGetVariableLengthStringTypeDeclarationSQLWithLength::<Case>();
}

#[test]
fn testItEscapesStringsForLike() {
    platforms_support::testItEscapesStringsForLike::<Case>();
}

#[test]
fn testKeywordList() {
    platforms_support::testKeywordList::<Case>();
}

#[test]
fn testQuoteStringLiteral() {
    platforms_support::testQuoteStringLiteral::<Case>();
}

#[test]
fn testQuotedColumnInForeignKeyPropagation() {
    platforms_support::testQuotedColumnInForeignKeyPropagation::<Case>();
}

#[test]
fn testQuotedColumnInIndexPropagation() {
    platforms_support::testQuotedColumnInIndexPropagation::<Case>();
}

#[test]
fn testQuotedColumnInPrimaryKeyPropagation() {
    platforms_support::testQuotedColumnInPrimaryKeyPropagation::<Case>();
}

#[test]
fn testQuotedNameInIndexSQL() {
    platforms_support::testQuotedNameInIndexSQL::<Case>();
}

#[test]
fn testQuotesAlterTableRenameIndex() {
    platforms_support::testQuotesAlterTableRenameIndex::<Case>();
}

#[test]
fn testQuotesAlterTableRenameIndexInSchema() {
    platforms_support::testQuotesAlterTableRenameIndexInSchema::<Case>();
}

#[test]
fn testQuotesReservedKeywordInIndexDeclarationSQL() {
    platforms_support::testQuotesReservedKeywordInIndexDeclarationSQL::<Case>();
}

#[test]
fn testQuotesReservedKeywordInTruncateTableSQL() {
    platforms_support::testQuotesReservedKeywordInTruncateTableSQL::<Case>();
}

#[test]
fn testQuotesReservedKeywordInUniqueConstraintDeclarationSQL() {
    platforms_support::testQuotesReservedKeywordInUniqueConstraintDeclarationSQL::<Case>();
}

#[test]
fn testRegisterDoctrineMappingType() {
    platforms_support::testRegisterDoctrineMappingType::<Case>();
}

#[test]
fn testRegisterUnknownDoctrineMappingType() {
    platforms_support::testRegisterUnknownDoctrineMappingType::<Case>();
}

#[test]
fn testReturnsForeignKeyReferentialActionSQL() {
    platforms_support::testReturnsForeignKeyReferentialActionSQL::<Case>();
}

#[test]
fn testReturnsJsonTypeDeclarationSQL() {
    platforms_support::testReturnsJsonTypeDeclarationSQL::<Case>();
}

#[test]
fn testReturnsJsonbTypeDeclarationSQL() {
    platforms_support::testReturnsJsonbTypeDeclarationSQL::<Case>();
}

#[test]
fn testSupportsCommentOnStatement() {
    platforms_support::testSupportsCommentOnStatement::<Case>();
}

#[test]
fn testThrowsExceptionOnGeneratingInlineColumnCommentSQLIfUnsupported() {
    platforms_support::testThrowsExceptionOnGeneratingInlineColumnCommentSQLIfUnsupported::<Case>();
}

#[test]
fn testZeroOffsetWithoutLimitIsIgnored() {
    platforms_support::testZeroOffsetWithoutLimitIsIgnored::<Case>();
}

#[test]
fn testDoesNotSupportRegexp() {
    let e = platform().get_regexp_expression().unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

#[test]
fn testGeneratesSqlSnippets() {
    let platform = platform();
    assert_eq!(platform.get_current_date_sql(), "CONVERT(date, GETDATE())");
    assert_eq!(platform.get_current_time_sql(), "CONVERT(time, GETDATE())");
    assert_eq!(platform.get_current_timestamp_sql(), "CURRENT_TIMESTAMP");
    assert_eq!(
        platform.get_concat_expression(&["column1", "column2", "column3"]),
        "CONCAT(column1, column2, column3)"
    );
}

#[test]
fn testGeneratesTransactionsCommands() {
    let platform = platform();
    for (level, sql) in [
        (
            TransactionIsolationLevel::ReadUncommitted,
            "READ UNCOMMITTED",
        ),
        (TransactionIsolationLevel::ReadCommitted, "READ COMMITTED"),
        (TransactionIsolationLevel::RepeatableRead, "REPEATABLE READ"),
        (TransactionIsolationLevel::Serializable, "SERIALIZABLE"),
    ] {
        assert_eq!(
            platform.get_set_transaction_isolation_sql(level).unwrap(),
            format!("SET TRANSACTION ISOLATION LEVEL {sql}")
        );
    }
}

#[test]
fn testGeneratesDDLSnippets() {
    let platform = platform();
    let drop_database_expectation = "DROP DATABASE foobar";

    assert_eq!(
        platform.get_list_databases_sql().unwrap(),
        "SELECT * FROM sys.databases"
    );
    assert_eq!(
        platform.get_create_database_sql("foobar").unwrap(),
        "CREATE DATABASE foobar"
    );
    assert_eq!(
        platform.get_drop_database_sql("foobar").unwrap(),
        drop_database_expectation
    );
    assert_eq!(platform.get_drop_table_sql("foobar"), "DROP TABLE foobar");
}

#[test]
fn testGeneratesTypeDeclarationForIntegers() {
    let platform = platform();
    assert_eq!(
        platform
            .get_integer_type_declaration_sql(&php_array![])
            .unwrap(),
        sql("INT")
    );
    assert_eq!(
        platform
            .get_integer_type_declaration_sql(&php_array!["autoincrement" => true])
            .unwrap(),
        sql("INT IDENTITY")
    );
    assert_eq!(
        platform
            .get_integer_type_declaration_sql(
                &php_array!["autoincrement" => true, "primary" => true]
            )
            .unwrap(),
        sql("INT IDENTITY")
    );
}

#[test]
fn testGeneratesTypeDeclarationsForStrings() {
    let platform = platform();
    assert_eq!(
        platform
            .get_clob_type_declaration_sql(&php_array![])
            .unwrap(),
        sql("VARCHAR(MAX)")
    );
    assert_eq!(
        platform
            .get_clob_type_declaration_sql(&php_array!["length" => 5, "fixed" => true])
            .unwrap(),
        sql("VARCHAR(MAX)")
    );
}

#[test]
fn testGeneratesTypeDeclarationsForAsciiStrings() {
    let platform = platform();
    assert_eq!(
        platform
            .get_ascii_string_type_declaration_sql(&php_array!["length" => 10, "fixed" => true])
            .unwrap(),
        sql("CHAR(10)")
    );
    assert_eq!(
        platform
            .get_ascii_string_type_declaration_sql(&php_array!["length" => 50])
            .unwrap(),
        sql("VARCHAR(50)")
    );
    assert_eq!(
        platform
            .get_ascii_string_type_declaration_sql(&php_array!["length" => 50, "fixed" => false])
            .unwrap(),
        sql("VARCHAR(50)")
    );
}

#[test]
fn testSupportsIdentityColumns() {
    assert!(platform().supports_identity_columns());
}

#[test]
fn testSupportsSchemas() {
    assert!(platform().supports_schemas());
}

#[test]
fn testDoesNotSupportSavePoints() {
    assert!(platform().supports_savepoints());
}

#[test]
fn testModifyLimitQuery() {
    assert_eq!(
        modify_limit_query("SELECT * FROM user", Some(10), 0),
        "SELECT * FROM user ORDER BY (SELECT 0) OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithEmptyOffset() {
    assert_eq!(
        modify_limit_query("SELECT * FROM user", Some(10), 0),
        "SELECT * FROM user ORDER BY (SELECT 0) OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithOffset() {
    assert_eq!(
        modify_limit_query("SELECT * FROM user ORDER BY username DESC", Some(10), 5),
        "SELECT * FROM user ORDER BY username DESC OFFSET 5 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithAscOrderBy() {
    assert_eq!(
        modify_limit_query("SELECT * FROM user ORDER BY username ASC", Some(10), 0),
        "SELECT * FROM user ORDER BY username ASC OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithLowercaseOrderBy() {
    assert_eq!(
        modify_limit_query("SELECT * FROM user order by username", Some(10), 0),
        "SELECT * FROM user order by username OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithDescOrderBy() {
    assert_eq!(
        modify_limit_query("SELECT * FROM user ORDER BY username DESC", Some(10), 0),
        "SELECT * FROM user ORDER BY username DESC OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithMultipleOrderBy() {
    assert_eq!(
        modify_limit_query(
            "SELECT * FROM user ORDER BY username DESC, usereamil ASC",
            Some(10),
            0
        ),
        "SELECT * FROM user ORDER BY username DESC, usereamil ASC OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithSubSelect() {
    assert_eq!(
        modify_limit_query(
            "SELECT * FROM (SELECT u.id as uid, u.name as uname) dctrn_result",
            Some(10),
            0
        ),
        "SELECT * FROM (SELECT u.id as uid, u.name as uname) dctrn_result \
         ORDER BY (SELECT 0) OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithSubSelectAndOrder() {
    assert_eq!(
        modify_limit_query(
            "SELECT * FROM (SELECT u.id as uid, u.name as uname) dctrn_result ORDER BY uname DESC",
            Some(10),
            0
        ),
        "SELECT * FROM (SELECT u.id as uid, u.name as uname) dctrn_result \
         ORDER BY uname DESC OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );

    assert_eq!(
        modify_limit_query(
            "SELECT * FROM (SELECT u.id, u.name) dctrn_result ORDER BY name DESC",
            Some(10),
            0
        ),
        "SELECT * FROM (SELECT u.id, u.name) dctrn_result ORDER BY name DESC \
         OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithSubSelectAndMultipleOrder() {
    assert_eq!(
        modify_limit_query(
            "SELECT * FROM (SELECT u.id as uid, u.name as uname) dctrn_result ORDER BY uname DESC, uid ASC",
            Some(10),
            5
        ),
        "SELECT * FROM (SELECT u.id as uid, u.name as uname) dctrn_result \
         ORDER BY uname DESC, uid ASC OFFSET 5 ROWS FETCH NEXT 10 ROWS ONLY"
    );

    assert_eq!(
        modify_limit_query(
            "SELECT * FROM (SELECT u.id uid, u.name uname) dctrn_result ORDER BY uname DESC, uid ASC",
            Some(10),
            5
        ),
        "SELECT * FROM (SELECT u.id uid, u.name uname) dctrn_result \
         ORDER BY uname DESC, uid ASC OFFSET 5 ROWS FETCH NEXT 10 ROWS ONLY"
    );

    assert_eq!(
        modify_limit_query(
            "SELECT * FROM (SELECT u.id, u.name) dctrn_result ORDER BY name DESC, id ASC",
            Some(10),
            5
        ),
        "SELECT * FROM (SELECT u.id, u.name) dctrn_result \
         ORDER BY name DESC, id ASC OFFSET 5 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithFromColumnNames() {
    assert_eq!(
        modify_limit_query("SELECT a.fromFoo, fromBar FROM foo", Some(10), 0),
        "SELECT a.fromFoo, fromBar FROM foo ORDER BY (SELECT 0) OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn testModifyLimitQueryWithExtraLongQuery() {
    let query = "SELECT table1.column1, table2.column2, table3.column3, table4.column4, \
        table5.column5, table6.column6, table7.column7, table8.column8 \
        FROM table1, table2, table3, table4, table5, table6, table7, table8 \
        WHERE (table1.column1 = table2.column2) AND (table1.column1 = table3.column3) \
        AND (table1.column1 = table4.column4) AND (table1.column1 = table5.column5) \
        AND (table1.column1 = table6.column6) AND (table1.column1 = table7.column7) \
        AND (table1.column1 = table8.column8) \
        AND (table2.column2 = table3.column3) AND (table2.column2 = table4.column4) \
        AND (table2.column2 = table5.column5) AND (table2.column2 = table6.column6) \
        AND (table2.column2 = table7.column7) AND (table2.column2 = table8.column8) \
        AND (table3.column3 = table4.column4) AND (table3.column3 = table5.column5) \
        AND (table3.column3 = table6.column6) AND (table3.column3 = table7.column7) \
        AND (table3.column3 = table8.column8) \
        AND (table4.column4 = table5.column5) AND (table4.column4 = table6.column6) \
        AND (table4.column4 = table7.column7) AND (table4.column4 = table8.column8) \
        AND (table5.column5 = table6.column6) AND (table5.column5 = table7.column7) \
        AND (table5.column5 = table8.column8) \
        AND (table6.column6 = table7.column7) AND (table6.column6 = table8.column8) \
        AND (table7.column7 = table8.column8)";

    let expected = format!("{query} ORDER BY (SELECT 0) OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY");

    assert_eq!(modify_limit_query(query, Some(10), 0), expected);
}

#[test]
fn testModifyLimitQueryWithOrderByClause() {
    let sql = "SELECT m0_.NOMBRE AS NOMBRE0, m0_.FECHAINICIO AS FECHAINICIO1, m0_.FECHAFIN AS FECHAFIN2 \
        FROM MEDICION m0_ \
        INNER JOIN ESTUDIO e1_ ON m0_.ESTUDIO_ID = e1_.ID \
        INNER JOIN CLIENTE c2_ ON e1_.CLIENTE_ID = c2_.ID \
        INNER JOIN USUARIO u3_ ON c2_.ID = u3_.CLIENTE_ID \
        WHERE u3_.ID = ? ORDER BY m0_.FECHAINICIO DESC";

    let expected = format!("{sql} OFFSET 5 ROWS FETCH NEXT 10 ROWS ONLY");

    assert_eq!(modify_limit_query(sql, Some(10), 5), expected);
}

const SUB_SELECT_IN_SELECT_LIST: &str = "SELECT \
    u.id, \
    (u.foo/2) foodiv, \
    CONCAT(u.bar, u.baz) barbaz, \
    (SELECT (SELECT COUNT(*) FROM login l WHERE l.profile_id = p.id) \
    FROM profile p WHERE p.user_id = u.id) login_count \
    FROM user u \
    WHERE u.status = 'disabled'";

#[test]
fn testModifyLimitQueryWithSubSelectInSelectList() {
    assert_eq!(
        modify_limit_query(SUB_SELECT_IN_SELECT_LIST, Some(10), 0),
        format!(
            "{SUB_SELECT_IN_SELECT_LIST} ORDER BY (SELECT 0) OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY"
        )
    );
}

#[test]
fn testModifyLimitQueryWithSubSelectInSelectListAndOrderByClause() {
    assert_eq!(
        modify_limit_query(
            &format!("{SUB_SELECT_IN_SELECT_LIST} ORDER BY u.username DESC"),
            Some(10),
            5
        ),
        format!(
            "{SUB_SELECT_IN_SELECT_LIST} ORDER BY u.username DESC OFFSET 5 ROWS FETCH NEXT 10 ROWS ONLY"
        )
    );
}

#[test]
fn testModifyLimitQueryWithAggregateFunctionInOrderByClause() {
    let query = "SELECT MAX(heading_id) aliased, code FROM operator_model_operator \
        GROUP BY code ORDER BY MAX(heading_id) DESC";
    assert_eq!(
        modify_limit_query(query, Some(1), 0),
        format!("{query} OFFSET 0 ROWS FETCH NEXT 1 ROWS ONLY")
    );
}

fn distinct_subquery(columns: &str, inner: &str, order_by: &str) -> String {
    format!(
        "SELECT DISTINCT {columns} FROM (SELECT {inner} FROM table_parent t1 \
         LEFT JOIN join_table t2 ON t1.id = t2.table_id) dctrn_result ORDER BY {order_by}"
    )
}

#[test]
fn testModifyLimitSubqueryWithJoinAndSubqueryOrderedByColumnFromBaseTable() {
    let query_sql = distinct_subquery(
        "id_0, name_1",
        "t1.id AS id_0, t2.name AS name_1",
        "id_0 ASC",
    );
    let altered_sql = format!("{query_sql} OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY");
    assert_eq!(modify_limit_query(&query_sql, Some(5), 0), altered_sql);
}

#[test]
fn testModifyLimitSubqueryWithJoinAndSubqueryOrderedByColumnFromJoinTable() {
    let query_sql = distinct_subquery(
        "id_0, name_1",
        "t1.id AS id_0, t2.name AS name_1",
        "name_1 ASC",
    );
    let altered_sql = format!("{query_sql} OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY");
    assert_eq!(modify_limit_query(&query_sql, Some(5), 0), altered_sql);
}

#[test]
fn testModifyLimitSubqueryWithJoinAndSubqueryOrderedByColumnsFromBothTables() {
    let query_sql = distinct_subquery(
        "id_0, name_1, foo_2",
        "t1.id AS id_0, t2.name AS name_1, t2.foo AS foo_2",
        "name_1 ASC, foo_2 DESC",
    );
    let altered_sql = format!("{query_sql} OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY");
    assert_eq!(modify_limit_query(&query_sql, Some(5), 0), altered_sql);
}

#[test]
fn testModifyLimitSubquerySimple() {
    let query = "SELECT DISTINCT id_0 FROM (\
        SELECT k0_.id AS id_0, k0_.column AS column_1 FROM key_table k0_ WHERE (k0_.where_column IN (1))\
        ) dctrn_result";

    let expected = format!("{query} ORDER BY 1 OFFSET 0 ROWS FETCH NEXT 20 ROWS ONLY");

    assert_eq!(modify_limit_query(query, Some(20), 0), expected);
}

#[test]
fn testQuoteIdentifier() {
    assert_eq!(platform().quote_identifier("test.test"), "[test].[test]");
}

#[test]
fn testCreateClusteredIndex() {
    let mut idx = index("idx", &["id"], false, false);
    idx.add_flag("clustered");
    assert_eq!(
        platform().get_create_index_sql(&idx, "tbl").unwrap(),
        "CREATE CLUSTERED INDEX idx ON tbl (id)"
    );
}

#[test]
fn testCreateNonClusteredPrimaryKeyInTable() {
    let table = Table::editor()
        .set_unquoted_name("tbl", None)
        .set_columns(vec![integer_column("id")])
        .set_primary_key_constraint(Some(
            PrimaryKeyConstraint::editor()
                .set_unquoted_column_names(&["id"])
                .set_is_clustered(false)
                .create()
                .unwrap(),
        ))
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        vec!["CREATE TABLE tbl (id INT NOT NULL, PRIMARY KEY NONCLUSTERED (id))"]
    );
}

#[test]
fn testCreateNonClusteredPrimaryKey() {
    let mut idx = index("idx", &["id"], false, true);
    idx.add_flag("nonclustered");
    assert_eq!(
        platform().get_create_primary_key_sql(&idx, "tbl").unwrap(),
        "ALTER TABLE tbl ADD PRIMARY KEY NONCLUSTERED (id)"
    );
}

#[test]
fn testAlterAddPrimaryKey() {
    let idx = index("idx", &["id"], false, true);
    assert_eq!(
        platform().get_create_index_sql(&idx, "tbl").unwrap(),
        "ALTER TABLE tbl ADD PRIMARY KEY (id)"
    );
}

#[test]
fn testGetCreateSchemaSQL() {
    let schema_name = "schema";
    let sql = platform().get_create_schema_sql(schema_name).unwrap();
    assert_eq!(sql, format!("CREATE SCHEMA {schema_name}"));
}

#[test]
fn testCreateTableWithSchemaColumnComments() {
    let table = Table::editor()
        .set_unquoted_name("test", Some("testschema"))
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_comment("This is a comment")
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(pk(&["id"]))
        .create()
        .unwrap();

    let expected_sql = vec![
        "CREATE TABLE testschema.test (id INT NOT NULL, PRIMARY KEY (id))",
        "EXEC [sp_addextendedproperty] N'MS_Description', N'This is a comment', \
         N'SCHEMA', 'testschema', N'TABLE', 'test', N'COLUMN', 'id'",
    ];

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        expected_sql
    );
}

#[test]
fn testAlterTableWithSchemaColumnComments() {
    let table_diff = TableDiff::new(
        schema_mytable(),
        TableDiffParts {
            added_columns: vec![
                column("quota", Types::INTEGER)
                    .set_comment("A comment")
                    .create()
                    .unwrap(),
            ],
            ..TableDiffParts::default()
        },
    );

    let expected_sql = vec![
        "ALTER TABLE testschema.mytable ADD quota INT NOT NULL",
        "EXEC [sp_addextendedproperty] N'MS_Description', N'A comment', \
         N'SCHEMA', 'testschema', N'TABLE', 'mytable', N'COLUMN', 'quota'",
    ];

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        expected_sql
    );
}

#[test]
fn testAlterTableWithSchemaDropColumnComments() {
    let table_diff = changed_quota(
        column("quota", Types::INTEGER)
            .set_comment("A comment")
            .create()
            .unwrap(),
        integer_column("quota"),
    );

    let expected_sql = vec![
        "EXEC [sp_dropextendedproperty] N'MS_Description', \
         N'SCHEMA', 'testschema', N'TABLE', 'mytable', N'COLUMN', 'quota'",
    ];

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        expected_sql
    );
}

#[test]
fn testAlterTableWithSchemaUpdateColumnComments() {
    let table_diff = changed_quota(
        column("quota", Types::INTEGER)
            .set_comment("A comment")
            .create()
            .unwrap(),
        column("quota", Types::INTEGER)
            .set_comment("B comment")
            .create()
            .unwrap(),
    );

    let expected_sql = vec![
        "EXEC [sp_updateextendedproperty] N'MS_Description', N'B comment', \
         N'SCHEMA', 'testschema', N'TABLE', 'mytable', N'COLUMN', 'quota'",
    ];

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        expected_sql
    );
}

#[test]
fn testInitializesDoctrineTypeMappings() {
    let platform = platform();
    for (db_type, doctrine_type) in [
        ("bigint", Types::BIGINT),
        ("numeric", Types::DECIMAL),
        ("bit", Types::BOOLEAN),
        ("smallint", Types::SMALLINT),
        ("decimal", Types::DECIMAL),
        ("smallmoney", Types::INTEGER),
        ("int", Types::INTEGER),
        ("tinyint", Types::SMALLINT),
        ("money", Types::INTEGER),
        ("float", Types::FLOAT),
        ("real", Types::SMALLFLOAT),
        ("double", Types::FLOAT),
        ("double precision", Types::FLOAT),
        ("smalldatetime", Types::DATETIME_MUTABLE),
        ("datetime", Types::DATETIME_MUTABLE),
        ("char", Types::STRING),
        ("varchar", Types::STRING),
        ("text", Types::TEXT),
        ("nchar", Types::STRING),
        ("nvarchar", Types::STRING),
        ("ntext", Types::TEXT),
        ("binary", Types::BINARY),
        ("varbinary", Types::BINARY),
        ("image", Types::BLOB),
        ("uniqueidentifier", Types::GUID),
        ("sysname", Types::STRING),
        ("xml", Types::TEXT),
    ] {
        assert!(
            platform.has_doctrine_type_mapping_for(db_type).unwrap(),
            "{db_type}"
        );
        assert_eq!(
            platform.get_doctrine_type_mapping(db_type).unwrap(),
            doctrine_type,
            "{db_type}"
        );
    }
}

#[test]
fn testGetVariableLengthStringTypeDeclarationSQLNoLength() {
    assert_invalid_column_declaration(
        platform().get_string_type_declaration_sql(&php_array!["name" => "email"]),
    );
}

#[test]
fn testGetVariableLengthBinaryTypeDeclarationSQLNoLength() {
    assert_invalid_column_declaration(
        platform().get_binary_type_declaration_sql(&php_array!["name" => "attachment"]),
    );
}

#[test]
fn testReturnsGuidTypeDeclarationSQL() {
    assert_eq!(
        platform()
            .get_guid_type_declaration_sql(&php_array![])
            .unwrap(),
        sql("UNIQUEIDENTIFIER")
    );
}

#[test]
fn testModifyLimitQueryWithTopNSubQueryWithOrderBy() {
    let query =
        "SELECT * FROM test t WHERE t.id = (SELECT TOP 1 t2.id FROM test t2 ORDER BY t2.data DESC)";
    assert_eq!(
        modify_limit_query(query, Some(10), 0),
        format!("{query} ORDER BY (SELECT 0) OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY")
    );

    let query = "SELECT * FROM test t WHERE t.id = (\
        SELECT TOP 1 t2.id FROM test t2 ORDER BY t2.data DESC\
        ) ORDER BY t.data2 DESC";
    assert_eq!(
        modify_limit_query(query, Some(10), 0),
        format!("{query} OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY")
    );
}

#[test]
fn testModifyLimitQueryWithFromSubquery() {
    let query = "SELECT DISTINCT id_0 FROM (\
        SELECT k0_.id AS id_0 FROM key_measure k0_ WHERE (k0_.id_zone in(2))\
        ) dctrn_result";
    assert_eq!(
        modify_limit_query(query, Some(10), 0),
        format!("{query} ORDER BY 1 OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY")
    );
}

#[test]
fn testModifyLimitQueryWithFromSubqueryAndOrder() {
    let query = "SELECT DISTINCT id_0, value_1 FROM (\
        SELECT k0_.id AS id_0, k0_.value AS value_1 FROM key_measure k0_ WHERE (k0_.id_zone in(2))\
        ) dctrn_result ORDER BY value_1 DESC";
    assert_eq!(
        modify_limit_query(query, Some(10), 0),
        format!("{query} OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY")
    );
}

#[test]
fn testModifyLimitQueryWithComplexOrderByExpression() {
    let sql = modify_limit_query(
        "SELECT * FROM table ORDER BY (table.x * table.y) DESC",
        Some(10),
        0,
    );
    let expected = "SELECT * FROM table ORDER BY (table.x * table.y) DESC OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY";
    assert_eq!(sql, expected);
}

#[test]
fn testModifyLimitQueryWithNewlineBeforeOrderBy() {
    let query_sql = "SELECT * FROM test\nORDER BY col DESC";
    let expected_sql =
        "SELECT * FROM test\nORDER BY col DESC OFFSET 0 ROWS FETCH NEXT 10 ROWS ONLY";
    assert_eq!(modify_limit_query(query_sql, Some(10), 0), expected_sql);
}

/// `'type' => Type::getType($type)` (the deprecated key, as upstream's
/// test passes it).
#[test]
fn testGetDefaultValueDeclarationSQLForDateType() {
    let platform = platform();
    let _current_date_sql = platform.get_current_date_sql();
    for type_ in [Types::DATE_MUTABLE, Types::DATE_IMMUTABLE] {
        let mut column = php_array![];
        column.insert("type", TypeObject::value(types::get_type(type_).unwrap()));
        column.insert("default", CurrentDate::value());
        assert_eq!(
            platform.get_default_value_declaration_sql(&column).unwrap(),
            " DEFAULT CONVERT(date, GETDATE())"
        );
    }
}

#[test]
fn testSupportsColumnCollation() {
    assert!(platform().supports_column_collation());
}

#[test]
fn testColumnCollationDeclarationSQL() {
    assert_eq!(
        platform().get_column_collation_declaration_sql("Latin1_General_CS_AS_KS_WS"),
        "COLLATE Latin1_General_CS_AS_KS_WS"
    );
}

#[test]
fn testGetCreateTableSQLWithColumnCollation() {
    let table = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![
            column("no_collation", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
            column("column_collation", Types::STRING)
                .set_length(Some(255))
                .set_collation(Some("Latin1_General_CS_AS_KS_WS"))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        vec![
            "CREATE TABLE foo (no_collation NVARCHAR(255) NOT NULL, \
             column_collation NVARCHAR(255) COLLATE Latin1_General_CS_AS_KS_WS NOT NULL)"
        ]
    );
}

#[test]
fn testSupportsSequences() {
    assert!(platform().supports_sequences());
}

#[test]
fn testGeneratesSequenceSqlCommands() {
    let platform = platform();
    let sequence = Sequence::editor()
        .set_unquoted_name("myseq", None)
        .set_allocation_size(20)
        .create()
        .unwrap();

    assert_eq!(
        platform.get_create_sequence_sql(&sequence).unwrap(),
        "CREATE SEQUENCE myseq START WITH 1 INCREMENT BY 20 MINVALUE 1"
    );
    assert_eq!(
        platform.get_alter_sequence_sql(&sequence).unwrap(),
        "ALTER SEQUENCE myseq INCREMENT BY 20"
    );
    assert_eq!(
        platform.get_drop_sequence_sql("myseq").unwrap(),
        "DROP SEQUENCE myseq"
    );
    assert_eq!(
        platform.get_sequence_next_val_sql("myseq").unwrap(),
        "SELECT NEXT VALUE FOR myseq"
    );
}

#[test]
fn testAlterTableWithSchemaSameColumnComments() {
    let table_diff = changed_quota(
        column("quota", Types::INTEGER)
            .set_comment("A comment")
            .set_not_null(false)
            .create()
            .unwrap(),
        column("quota", Types::INTEGER)
            .set_comment("A comment")
            .create()
            .unwrap(),
    );

    let expected_sql = vec!["ALTER TABLE testschema.mytable ALTER COLUMN quota INT NOT NULL"];

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        expected_sql
    );
}

/// `LockMode` from a recorded enum case.
fn lock_mode(json: &serde_json::Value) -> LockMode {
    assert_eq!(json[0], "enum", "not an enum: {json}");
    assert_eq!(
        json[1], "Doctrine\\DBAL\\LockMode",
        "not a LockMode: {json}"
    );
    match json[2].as_str().expect("case name") {
        "NONE" => LockMode::None,
        "OPTIMISTIC" => LockMode::Optimistic,
        "PESSIMISTIC_READ" => LockMode::PessimisticRead,
        "PESSIMISTIC_WRITE" => LockMode::PessimisticWrite,
        other => panic!("unknown LockMode case {other}"),
    }
}

#[test]
fn testAppendsLockHint() {
    let rows = rows::<Case>("testAppendsLockHint");
    assert_eq!(rows.len(), 4);
    for (name, args) in rows {
        let from_clause = "FROM users";
        let lock_hint = String::from_utf8(bytes(&args[1])).unwrap();
        let expected_result = format!("{from_clause}{lock_hint}");

        assert_eq!(
            platform().append_lock_hint(from_clause, lock_mode(&args[0])),
            expected_result,
            "data set {name}"
        );
    }
}

#[test]
fn testGeneratesTypeDeclarationForDateTimeTz() {
    assert_eq!(
        platform()
            .get_date_time_tz_type_declaration_sql(&php_array![])
            .unwrap(),
        sql("DATETIMEOFFSET(6)")
    );
}
