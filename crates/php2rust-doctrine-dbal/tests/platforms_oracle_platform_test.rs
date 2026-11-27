//! Port of tests/Platforms/OraclePlatformTest.php with the tests it
//! inherits from tests/Platforms/AbstractPlatformTestCase.php, at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. The test names
//! are PHPUnit's; an inherited test calls the shared port
//! (`tests/platforms_support`) on `Case`, whose impl holds this class's
//! expectations; `OraclePlatformTest`'s own and overridden tests are
//! written in place. The data providers' rows are the recorded ones
//! (tests/data/php.json).
//!
//! Upstream: doctrine/dbal tests/Platforms/OraclePlatformTest.php
//! Upstream: doctrine/dbal tests/Platforms/AbstractPlatformTestCase.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod platforms_support;

use std::time::{SystemTime, UNIX_EPOCH};

use common::{array, text};
use php_value::{PhpArray, Value, php_array};
use php2rust_doctrine_dbal::TransactionIsolationLevel;
use php2rust_doctrine_dbal::platforms::{AbstractPlatform, OraclePlatform};
use php2rust_doctrine_dbal::schema::{
    Column, Comparator, ComparatorConfig, ForeignKeyConstraint, Sequence, Table,
};
use php2rust_doctrine_dbal::types::Types;
use platforms_support::{
    AbstractPlatformTestCase, assert_invalid_column_declaration, column, rows, sql,
};

/// `OraclePlatformTest`.
struct Case;

impl AbstractPlatformTestCase for Case {
    type Platform = OraclePlatform;

    const PROVIDERS: &'static str = "Platforms/OraclePlatformTest";

    fn create_platform() -> OraclePlatform {
        OraclePlatform::new()
    }

    fn get_generate_table_sql() -> &'static str {
        "CREATE TABLE test (id NUMBER(10) NOT NULL, test VARCHAR2(255) DEFAULT NULL NULL, PRIMARY KEY (id))"
    }

    fn get_generate_table_with_multi_column_unique_index_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE test (foo VARCHAR2(255) DEFAULT NULL NULL, bar VARCHAR2(255) DEFAULT NULL NULL)",
            "CREATE UNIQUE INDEX UNIQ_D87F7E0C8C73652176FF8CAA ON test (foo, bar)",
        ]
    }

    fn supports_comment_on_statement() -> bool {
        true
    }

    fn get_generate_index_sql() -> &'static str {
        "CREATE INDEX my_idx ON mytable (user_name, last_login)"
    }

    fn get_generate_unique_index_sql() -> &'static str {
        "CREATE UNIQUE INDEX index_name ON test (test, test2)"
    }

    fn get_generate_foreign_key_sql() -> &'static str {
        "ALTER TABLE test ADD FOREIGN KEY (fk_name_id) REFERENCES other_table (id)"
    }

    fn get_bit_and_comparison_expression_sql(value1: &str, value2: &str) -> String {
        format!("BITAND({value1}, {value2})")
    }

    fn get_bit_or_comparison_expression_sql(value1: &str, value2: &str) -> String {
        format!(
            "({value1}-{}+{value2})",
            Self::get_bit_and_comparison_expression_sql(value1, value2)
        )
    }

    fn get_quoted_column_in_primary_key_sql() -> &'static [&'static str] {
        &["CREATE TABLE \"quoted\" (\"create\" VARCHAR2(255) NOT NULL, PRIMARY KEY (\"create\"))"]
    }

    fn get_quoted_column_in_index_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE \"quoted\" (\"create\" VARCHAR2(255) NOT NULL)",
            "CREATE INDEX IDX_22660D028FD6E0FB ON \"quoted\" (\"create\")",
        ]
    }

    fn get_quoted_name_in_index_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE test (column1 VARCHAR2(255) NOT NULL)",
            "CREATE INDEX \"key\" ON test (column1)",
        ]
    }

    fn get_quoted_column_in_foreign_key_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE \"quoted\" (\"create\" VARCHAR2(255) NOT NULL, foo VARCHAR2(255) NOT NULL, \
             \"bar\" VARCHAR2(255) NOT NULL)",
            "ALTER TABLE \"quoted\" ADD CONSTRAINT FK_WITH_RESERVED_KEYWORD FOREIGN KEY (\"create\", foo, \"bar\") \
             REFERENCES foreign (\"create\", bar, \"foo-bar\")",
            "ALTER TABLE \"quoted\" ADD CONSTRAINT FK_WITH_NON_RESERVED_KEYWORD FOREIGN KEY (\"create\", foo, \"bar\") \
             REFERENCES foo (\"create\", bar, \"foo-bar\")",
            "ALTER TABLE \"quoted\" ADD CONSTRAINT FK_WITH_INTENDED_QUOTATION FOREIGN KEY (\"create\", foo, \"bar\") \
             REFERENCES \"foo-bar\" (\"create\", bar, \"foo-bar\")",
            "CREATE INDEX IDX_22660D028FD6E0FB8C7365216D704F76 ON \"quoted\" (\"create\", foo, \"bar\")",
        ]
    }

    fn get_expected_variable_length_string_type_declaration_sql_with_length() -> &'static str {
        "VARCHAR2(16)"
    }

    fn get_expected_fixed_length_binary_type_declaration_sql_with_length() -> &'static str {
        "RAW(16)"
    }

    fn get_expected_variable_length_binary_type_declaration_sql_with_length() -> &'static str {
        "RAW(16)"
    }

    fn get_alter_table_rename_index_sql() -> &'static [&'static str] {
        &["ALTER INDEX idx_foo RENAME TO idx_bar"]
    }

    fn get_quoted_alter_table_rename_index_sql() -> &'static [&'static str] {
        &[
            "ALTER INDEX \"create\" RENAME TO \"select\"",
            "ALTER INDEX \"foo\" RENAME TO \"bar\"",
        ]
    }

    fn get_alter_table_rename_index_in_schema_sql() -> &'static [&'static str] {
        &["ALTER INDEX myschema.idx_foo RENAME TO idx_bar"]
    }

    fn get_quoted_alter_table_rename_index_in_schema_sql() -> &'static [&'static str] {
        &[
            "ALTER INDEX \"schema\".\"create\" RENAME TO \"select\"",
            "ALTER INDEX \"schema\".\"foo\" RENAME TO \"bar\"",
        ]
    }

    fn get_comment_on_column_sql() -> &'static [&'static str] {
        &[
            "COMMENT ON COLUMN foo.bar IS 'comment'",
            "COMMENT ON COLUMN \"Foo\".\"BAR\" IS 'comment'",
            "COMMENT ON COLUMN \"select\".\"from\" IS 'comment'",
        ]
    }

    fn get_quotes_reserved_keyword_in_unique_constraint_declaration_sql() -> &'static str {
        "CONSTRAINT \"select\" UNIQUE (foo)"
    }

    fn get_quotes_reserved_keyword_in_index_declaration_sql() -> &'static str {
        "INDEX \"select\" (foo)"
    }

    fn get_quotes_reserved_keyword_in_truncate_table_sql() -> &'static str {
        "TRUNCATE TABLE \"select\""
    }

    fn get_alter_string_to_fixed_string_sql() -> &'static [&'static str] {
        &["ALTER TABLE mytable MODIFY (name CHAR(2) DEFAULT NULL)"]
    }

    fn get_generates_alter_table_rename_index_used_by_foreign_key_sql() -> &'static [&'static str] {
        &["ALTER INDEX idx_foo RENAME TO idx_foo_renamed"]
    }
}

fn platform() -> OraclePlatform {
    Case::create_platform()
}

/// `createComparator()`.
fn create_comparator(platform: &OraclePlatform) -> Comparator<'_> {
    Comparator::new(platform, ComparatorConfig::default())
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
fn testGetCreateSchemaSQL() {
    platforms_support::testGetCreateSchemaSQL::<Case>();
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
fn testGetDefaultValueDeclarationSQLForDateType() {
    platforms_support::testGetDefaultValueDeclarationSQLForDateType::<Case>();
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
fn testQuoteIdentifier() {
    platforms_support::testQuoteIdentifier::<Case>();
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

// OraclePlatformTest's own and overridden tests.

#[test]
fn testRLike() {
    let e = platform().get_regexp_expression().unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

#[test]
fn testGeneratesSqlSnippets() {
    assert_eq!(
        platform().get_concat_expression(&["column1", "column2", "column3"]),
        "column1 || column2 || column3"
    );
}

#[test]
fn testGeneratesTransactionsCommands() {
    let platform = platform();
    for (level, expected) in [
        (
            TransactionIsolationLevel::ReadUncommitted,
            "READ UNCOMMITTED",
        ),
        (TransactionIsolationLevel::ReadCommitted, "READ COMMITTED"),
        (TransactionIsolationLevel::RepeatableRead, "SERIALIZABLE"),
        (TransactionIsolationLevel::Serializable, "SERIALIZABLE"),
    ] {
        assert_eq!(
            platform.get_set_transaction_isolation_sql(level).unwrap(),
            format!("SET TRANSACTION ISOLATION LEVEL {expected}")
        );
    }
}

#[test]
fn testCreateDatabaseSQL() {
    assert_eq!(
        platform().get_create_database_sql("foobar").unwrap(),
        "CREATE USER foobar"
    );
}

#[test]
fn testDropDatabaseSQL() {
    assert_eq!(
        platform().get_drop_database_sql("foobar").unwrap(),
        "DROP USER foobar CASCADE"
    );
}

#[test]
fn testDropTable() {
    assert_eq!(platform().get_drop_table_sql("foobar"), "DROP TABLE foobar");
}

#[test]
fn testGeneratesTypeDeclarationForIntegers() {
    let platform = platform();
    for column in [
        PhpArray::new(),
        php_array!["autoincrement" => true],
        php_array!["autoincrement" => true, "primary" => true],
    ] {
        assert_eq!(
            platform.get_integer_type_declaration_sql(&column).unwrap(),
            sql("NUMBER(10)")
        );
    }
}

#[test]
fn testSupportsIdentityColumns() {
    assert!(!platform().supports_identity_columns());
}

#[test]
fn testSupportsSavePoints() {
    assert!(platform().supports_savepoints());
}

#[test]
fn testGeneratesAdvancedForeignKeyOptionsSQL() {
    for (name, args) in rows::<Case>("testGeneratesAdvancedForeignKeyOptionsSQL") {
        let foreign_key =
            ForeignKeyConstraint::new(["foo"], "foreign_table", ["bar"], "", array(&args[0]))
                .unwrap();
        assert_eq!(
            platform()
                .get_advanced_foreign_key_options_sql(&foreign_key)
                .unwrap(),
            text(&args[1]),
            "{name}"
        );
    }
}

/// `uniqid()`: the time in hexadecimal seconds and microseconds.
fn uniqid() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{:08x}{:05x}", now.as_secs(), now.subsec_micros())
}

#[test]
fn testGenerateTableWithAutoincrement() {
    let column_name = format!("id{}", uniqid()).to_uppercase();
    let table_name = format!("table{}", uniqid()).to_uppercase();
    let table = Table::editor()
        .set_unquoted_name(&table_name, None)
        .set_columns(vec![
            column(&column_name, Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        vec![
            format!("CREATE TABLE {table_name} ({column_name} NUMBER(10) NOT NULL)"),
            format!(
                "DECLARE
  CONSTRAINTS_COUNT NUMBER;
BEGIN
  SELECT COUNT(CONSTRAINT_NAME) INTO CONSTRAINTS_COUNT
    FROM USER_CONSTRAINTS
   WHERE TABLE_NAME = '{table_name}'
     AND CONSTRAINT_TYPE = 'P';
  IF CONSTRAINTS_COUNT = 0 THEN
    EXECUTE IMMEDIATE 'ALTER TABLE {table_name} ADD CONSTRAINT {table_name}_AI_PK PRIMARY KEY ({column_name})';
  END IF;
END;"
            ),
            format!("CREATE SEQUENCE {table_name}_SEQ START WITH 1 MINVALUE 1 INCREMENT BY 1"),
            format!(
                "CREATE TRIGGER {table_name}_AI_PK
   BEFORE INSERT
   ON {table_name}
   FOR EACH ROW
DECLARE
   last_Sequence NUMBER;
   last_InsertID NUMBER;
BEGIN
   IF (:NEW.{column_name} IS NULL OR :NEW.{column_name} = 0) THEN
      SELECT {table_name}_SEQ.NEXTVAL INTO :NEW.{column_name} FROM DUAL;
   ELSE
      SELECT NVL(Last_Number, 0) INTO last_Sequence
        FROM USER_SEQUENCES
       WHERE Sequence_Name = '{table_name}_SEQ';
      SELECT :NEW.{column_name} INTO last_InsertID FROM DUAL;
      WHILE (last_InsertID > last_Sequence) LOOP
         SELECT {table_name}_SEQ.NEXTVAL INTO last_Sequence FROM DUAL;
      END LOOP;
   END IF;
END;"
            ),
        ]
    );
}

#[test]
fn testInitializesDoctrineTypeMappings() {
    let platform = platform();
    for (db_type, expected) in [
        ("long raw", Types::BLOB),
        ("raw", Types::BINARY),
        ("date", Types::DATE_MUTABLE),
        ("real", Types::SMALLFLOAT),
    ] {
        assert!(platform.has_doctrine_type_mapping_for(db_type).unwrap());
        assert_eq!(
            platform.get_doctrine_type_mapping(db_type).unwrap(),
            expected
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
fn testGetFixedLengthBinaryTypeDeclarationSQLNoLength() {
    assert_invalid_column_declaration(
        platform()
            .get_binary_type_declaration_sql(&php_array!["name" => "checksum", "fixed" => true]),
    );
}

#[test]
fn testGetVariableLengthBinaryTypeDeclarationSQLNoLength() {
    assert_invalid_column_declaration(
        platform().get_binary_type_declaration_sql(&php_array!["name" => "attachment"]),
    );
}

#[test]
fn testDoesNotPropagateUnnecessaryTableAlterationOnBinaryType() {
    let platform = platform();
    let binary = |name: &str, fixed: bool| {
        let mut editor = column(name, Types::BINARY);
        if fixed {
            editor = editor.set_fixed(true);
        }
        editor.set_length(Some(32)).create().unwrap()
    };
    let mytable = |columns: Vec<Column>| {
        Table::editor()
            .set_unquoted_name("mytable", None)
            .set_columns(columns)
            .create()
            .unwrap()
    };

    let table1 = mytable(vec![
        binary("column_varbinary", false),
        binary("column_binary", true),
    ]);
    let table2 = mytable(vec![
        binary("column_varbinary", true),
        binary("column_binary", false),
    ]);

    assert!(
        create_comparator(&platform)
            .compare_tables(&table1, &table2)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn testCreateSequenceWithCache() {
    for (name, args) in rows::<Case>("testCreateSequenceWithCache") {
        let Value::Int(cache_size) = php_value::vectors::decode(&args[0]).unwrap() else {
            panic!("{name}");
        };
        let sequence = Sequence::editor()
            .set_unquoted_name("foo", None)
            .set_cache_size(Some(cache_size))
            .create()
            .unwrap();
        let sql = platform().get_create_sequence_sql(&sequence).unwrap();
        assert!(sql.contains(&text(&args[1])), "{name}: {sql}");
    }
}

#[test]
fn testReturnsGuidTypeDeclarationSQL() {
    assert_eq!(
        platform()
            .get_guid_type_declaration_sql(&PhpArray::new())
            .unwrap(),
        sql("CHAR(36)")
    );
}

#[test]
fn testReturnsDropAutoincrementSQL() {
    for (name, args) in rows::<Case>("testReturnsDropAutoincrementSQL") {
        let expected: Vec<String> = array(&args[1])
            .iter()
            .map(|(_, value)| {
                value
                    .to_php_string()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            platform()
                .get_drop_autoincrement_sql(&text(&args[0]))
                .unwrap(),
            expected,
            "{name}"
        );
    }
}

#[test]
fn testAltersTableColumnCommentWithExplicitlyQuotedIdentifiers() {
    let platform = platform();
    let foo_bar = |comment: Option<&str>| {
        let mut editor = Column::editor()
            .set_quoted_name("Bar")
            .set_type_name(Types::INTEGER);
        if let Some(comment) = comment {
            editor = editor.set_comment(comment);
        }
        Table::editor()
            .set_quoted_name("Foo", None)
            .set_columns(vec![editor.create().unwrap()])
            .create()
            .unwrap()
    };
    let table1 = foo_bar(None);
    let table2 = foo_bar(Some("Baz"));

    let table_diff = create_comparator(&platform)
        .compare_tables(&table1, &table2)
        .unwrap();

    assert_eq!(
        platform.get_alter_table_sql(&table_diff).unwrap(),
        vec!["COMMENT ON COLUMN \"Foo\".\"Bar\" IS 'Baz'"]
    );
}

#[test]
fn testQuotedTableNames() {
    let platform = platform();
    let table = Table::editor()
        .set_quoted_name("test", None)
        .set_columns(vec![
            Column::editor()
                .set_quoted_name("id")
                .set_type_name(Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert!(table.is_quoted());
    assert_eq!(
        table
            .get_object_name()
            .unwrap()
            .get_unqualified_name()
            .get_value(),
        "test"
    );
    assert_eq!(table.get_quoted_name(&platform), "\"test\"");

    let sql = platform.get_create_table_sql(&table).unwrap();
    assert_eq!(sql[0], "CREATE TABLE \"test\" (\"id\" NUMBER(10) NOT NULL)");
    assert_eq!(
        sql[2],
        "CREATE SEQUENCE \"test_SEQ\" START WITH 1 MINVALUE 1 INCREMENT BY 1"
    );

    let create_trigger_statement = "CREATE TRIGGER \"test_AI_PK\"
   BEFORE INSERT
   ON \"test\"
   FOR EACH ROW
DECLARE
   last_Sequence NUMBER;
   last_InsertID NUMBER;
BEGIN
   IF (:NEW.\"id\" IS NULL OR :NEW.\"id\" = 0) THEN
      SELECT \"test_SEQ\".NEXTVAL INTO :NEW.\"id\" FROM DUAL;
   ELSE
      SELECT NVL(Last_Number, 0) INTO last_Sequence
        FROM USER_SEQUENCES
       WHERE Sequence_Name = 'test_SEQ';
      SELECT :NEW.\"id\" INTO last_InsertID FROM DUAL;
      WHILE (last_InsertID > last_Sequence) LOOP
         SELECT \"test_SEQ\".NEXTVAL INTO last_Sequence FROM DUAL;
      END LOOP;
   END IF;
END;";

    assert_eq!(sql[3], create_trigger_statement);
}

#[test]
fn testQuotesDatabaseNameInListSequencesSQL() {
    let sql = platform().get_list_sequences_sql("Foo'Bar\\").unwrap();
    assert!(
        sql.to_lowercase().contains(&"'Foo''Bar\\'".to_lowercase()),
        "{sql}"
    );
}
