//! Port of tests/Functional/Query/QueryBuilderTest.php on `pdo_sqlite` in
//! memory (a fresh database per test, so `tearDown()`'s roll back has
//! nothing to do). `prepareExpectedRows()` keeps the rows as they are on
//! `pdo_sqlite` without stringified fetches. On `SQLite` upstream skips
//! `testForUpdateOrdinary`, `testForUpdateSkipLockedWhenSupported` and
//! `testPlatformDoesNotSupportCTE`.
//!
//! Upstream: doctrine/dbal tests/Functional/Query/QueryBuilderTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::query::{ConflictResolutionMode, QueryBuilder, UnionType};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{
    ArrayParameterType, Connection, ConnectionStandIn, Exception, ParameterType, ParameterTypes,
};
use schema_manager_support::{column, drop_and_create_table, platform, primary_key, sqlite};

/// `setUp()`: the `for_update` table with the ids 1 and 2.
async fn set_up() -> Connection {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("for_update", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    for id in [1, 2] {
        let mut data = PhpArray::new();
        data.insert("id", Value::Int(id));
        connection
            .insert("for_update", &data, &ParameterTypes::new())
            .await
            .unwrap();
    }

    connection
}

/// A row of the expected rows: column name and integer value.
fn row(columns: &[(&str, i64)]) -> PhpArray {
    let mut row = PhpArray::new();
    for (name, value) in columns {
        row.insert(*name, Value::Int(*value));
    }
    row
}

/// `prepareExpectedRows()` on `pdo_sqlite`: the rows as they are.
fn prepare_expected_rows(rows: Vec<PhpArray>) -> Vec<PhpArray> {
    rows
}

/// `$qb->executeQuery()->fetchAllAssociative()`.
async fn fetch_all_associative(qb: &QueryBuilder<'_, Connection>) -> Vec<PhpArray> {
    qb.execute_query()
        .await
        .unwrap()
        .fetch_all_associative()
        .unwrap()
}

/// `$platform->getDummySelectSQL($expression)`.
async fn dummy(connection: &Connection, expression: &str) -> String {
    platform(connection).await.get_dummy_select_sql(expression)
}

/// `platformSupportsSkipLocked()`: not on `SQLite`.
fn platform_supports_skip_locked() -> bool {
    false
}

#[tokio::test]
async fn testForUpdateSkipLockedWhenNotSupported() {
    assert!(!platform_supports_skip_locked());
    let connection = set_up().await;

    let mut qb = connection.create_query_builder();
    qb.select(&["id"])
        .from("for_update", None)
        .for_update(ConflictResolutionMode::SkipLocked);

    let e: Exception = qb.execute_query().await.err().unwrap();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

/// A UNION of three dummy selects.
async fn union_of_dummies(
    connection: &Connection,
    values: [i64; 3],
    union_type: Option<UnionType>,
) -> QueryBuilder<'_, Connection> {
    let mut qb = connection.create_query_builder();
    qb.union(dummy(connection, &format!("{} as field_one", values[0])).await);
    for value in &values[1..] {
        qb.add_union(
            dummy(connection, &format!("{value} as field_one")).await,
            union_type.unwrap_or_default(),
        )
        .unwrap();
    }
    qb
}

#[tokio::test]
async fn testUnionAllReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![
        row(&[("field_one", 1)]),
        row(&[("field_one", 1)]),
        row(&[("field_one", 2)]),
    ]);
    let mut qb = union_of_dummies(&connection, [2, 1, 1], Some(UnionType::All)).await;
    qb.order_by("field_one", Some("ASC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionDistinctReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows =
        prepare_expected_rows(vec![row(&[("field_one", 1)]), row(&[("field_one", 2)])]);
    let mut qb = union_of_dummies(&connection, [2, 1, 1], Some(UnionType::Distinct)).await;
    qb.order_by("field_one", Some("ASC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionIsDistinctByDefault() {
    let connection = set_up().await;
    let expected_rows =
        prepare_expected_rows(vec![row(&[("field_one", 1)]), row(&[("field_one", 2)])]);
    let mut qb = union_of_dummies(&connection, [2, 1, 1], None).await;
    qb.order_by("field_one", Some("ASC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionWithDescOrderByReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows =
        prepare_expected_rows(vec![row(&[("field_one", 2)]), row(&[("field_one", 1)])]);
    let mut qb = union_of_dummies(&connection, [1, 2, 1], Some(UnionType::Distinct)).await;
    qb.order_by("field_one", Some("DESC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionAllWithLimitClauseReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("field_one", 2)])]);
    let mut qb = union_of_dummies(&connection, [1, 2, 1], Some(UnionType::All)).await;
    qb.set_max_results(Some(1))
        .set_first_result(0)
        .order_by("field_one", Some("DESC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionWithLimitClauseReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("field_one", 2)])]);
    let mut qb = union_of_dummies(&connection, [1, 2, 1], Some(UnionType::Distinct)).await;
    qb.set_max_results(Some(1))
        .set_first_result(0)
        .order_by("field_one", Some("DESC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionAllWithLimitAndOffsetClauseReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("field_one", 1)])]);
    let mut qb = union_of_dummies(&connection, [1, 2, 1], Some(UnionType::All)).await;
    qb.set_max_results(Some(1))
        .set_first_result(1)
        .order_by("field_one", Some("ASC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionWithLimitAndOffsetClauseReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("field_one", 2)])]);
    let _plain_select1 = dummy(&connection, "1 as field_one").await;
    let _plain_select2 = dummy(&connection, "2 as field_one").await;
    let _plain_select3 = dummy(&connection, "1 as field_one").await;
    let mut qb = union_of_dummies(&connection, [1, 2, 1], Some(UnionType::Distinct)).await;
    qb.set_max_results(Some(1))
        .set_first_result(1)
        .order_by("field_one", Some("ASC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

/// `$qb->sub()->select('id')->from('for_update')->where($predicate)`.
fn sub_select_id<'c>(
    qb: &QueryBuilder<'c, Connection>,
    predicate: &str,
) -> QueryBuilder<'c, Connection> {
    let mut sub = qb.sub();
    sub.select(&["id"])
        .from("for_update", None)
        .r#where(predicate, []);
    sub
}

#[tokio::test]
async fn testUnionAllAndAddUnionAllWorksWithQueryBuilderPartsAndOrderByDescAndReturnsExpectedResult()
 {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![
        row(&[("id", 2)]),
        row(&[("id", 1)]),
        row(&[("id", 1)]),
    ]);
    let mut qb = connection.create_query_builder();

    let sub_query_builder1 = sub_select_id(&qb, &qb.expr().eq("id", "1"));
    let sub_query_builder2 = sub_select_id(&qb, &qb.expr().eq("id", "2"));
    let sub_query_builder3 = sub_select_id(&qb, &qb.expr().eq("id", "1"));

    qb.union(sub_query_builder1)
        .add_union(sub_query_builder2, UnionType::All)
        .unwrap()
        .add_union(sub_query_builder3, UnionType::All)
        .unwrap()
        .order_by("id", Some("DESC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

/// The three sub-queries of the UNION tests whose ids are named
/// parameters of the outer builder (1, 2 and 1).
fn named_parameter_sub_queries<'c>(
    qb: &mut QueryBuilder<'c, Connection>,
) -> [QueryBuilder<'c, Connection>; 3] {
    [1, 2, 1].map(|id| {
        let placeholder = qb.create_named_parameter(Value::Int(id), ParameterType::Integer, None);
        sub_select_id(qb, &qb.expr().eq("id", &placeholder))
    })
}

#[tokio::test]
async fn testUnionAndAddUnionWithNamedParameterOnOuterInstanceAndOrderByDescWorks() {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("id", 2)]), row(&[("id", 1)])]);
    let mut qb = connection.create_query_builder();

    let [sub_query_builder1, sub_query_builder2, sub_query_builder3] =
        named_parameter_sub_queries(&mut qb);

    qb.union(sub_query_builder1)
        .add_union(sub_query_builder2, UnionType::Distinct)
        .unwrap()
        .add_union(sub_query_builder3, UnionType::Distinct)
        .unwrap()
        .order_by("id", Some("DESC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionAllAndAddUnionAllWorksWithQueryBuilderPartsAndReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![
        row(&[("id", 1)]),
        row(&[("id", 1)]),
        row(&[("id", 2)]),
    ]);
    let mut qb = connection.create_query_builder();

    let [sub_query_builder1, sub_query_builder2, sub_query_builder3] =
        named_parameter_sub_queries(&mut qb);

    qb.union(sub_query_builder1)
        .add_union(sub_query_builder2, UnionType::All)
        .unwrap()
        .add_union(sub_query_builder3, UnionType::All)
        .unwrap()
        .order_by("id", Some("ASC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testUnionAndAddUnionWorksWithQueryBuilderPartsAndReturnsExpectedResult() {
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("id", 1)]), row(&[("id", 2)])]);
    let mut qb = connection.create_query_builder();

    let [sub_query_builder1, sub_query_builder2, sub_query_builder3] =
        named_parameter_sub_queries(&mut qb);

    qb.union(sub_query_builder1)
        .add_union(sub_query_builder2, UnionType::Distinct)
        .unwrap()
        .add_union(sub_query_builder3, UnionType::Distinct)
        .unwrap()
        .order_by("id", Some("ASC"));

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

/// `platformSupportsCTEs()` and `platformSupportsCTEColumnsDefinition()`:
/// both on `SQLite`.
fn platform_supports_ctes() -> bool {
    true
}

fn platform_supports_cte_columns_definition() -> bool {
    true
}

/// `$qb->sub()->select('id AS virtual_id')->from('for_update')->where($predicate)`.
fn sub_select_virtual_id<'c>(
    qb: &QueryBuilder<'c, Connection>,
    predicate: &str,
) -> QueryBuilder<'c, Connection> {
    let mut sub = qb.sub();
    sub.select(&["id AS virtual_id"])
        .from("for_update", None)
        .r#where(predicate, []);
    sub
}

/// The three CTEs and the joins of the CTE parameter tests, selecting
/// `c.virtual_id` where `a.virtual_id = ?`.
fn with_three_ctes<'c>(
    qb: &mut QueryBuilder<'c, Connection>,
    cte_query_builder1: QueryBuilder<'c, Connection>,
    cte_query_builder2: QueryBuilder<'c, Connection>,
) {
    let predicate = qb.expr().eq("a.virtual_id", "?");
    qb.with("cte_a", cte_query_builder1, Some(&["virtual_id"]))
        .unwrap()
        .with("cte_b", cte_query_builder2, Some(&["virtual_id"]))
        .unwrap()
        .with("cte_c", "SELECT 1 AS virtual_id", None)
        .unwrap()
        .select(&["c.virtual_id"])
        .from("cte_a", Some("a"))
        .join("a", "cte_b", "b", Some("a.virtual_id = b.virtual_id"))
        .join("b", "cte_c", "c", Some("b.virtual_id = c.virtual_id"))
        .r#where(predicate, []);
}

/// `[1, 2]`.
fn one_two() -> Value {
    Value::Array(PhpArray::list([Value::Int(1), Value::Int(2)]))
}

#[tokio::test]
async fn testSelectWithCTENamedParameter() {
    assert!(platform_supports_ctes() && platform_supports_cte_columns_definition());
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("virtual_id", 1)])]);
    let mut qb = connection.create_query_builder();

    let cte_query_builder = sub_select_virtual_id(&qb, "id = :id");

    qb.with("cte_a", cte_query_builder, Some(&["virtual_id"]))
        .unwrap()
        .select(&["virtual_id"])
        .from("cte_a", None)
        .set_parameter("id", Value::Int(1), ParameterType::String);

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testSelectWithCTEAndParametersBindForEachQuery() {
    assert!(platform_supports_ctes() && platform_supports_cte_columns_definition());
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("virtual_id", 1)])]);
    let mut qb = connection.create_query_builder();

    let cte_query_builder1 = sub_select_virtual_id(&qb, &qb.expr().eq("id", "?"));
    qb.set_parameter(0, Value::Int(1), ParameterType::Integer);

    let cte_query_builder2 = sub_select_virtual_id(&qb, &qb.expr().r#in("id", &[":id"]));
    qb.set_parameter("id", one_two(), ArrayParameterType::Integer);

    with_three_ctes(&mut qb, cte_query_builder1, cte_query_builder2);
    qb.set_parameter(1, Value::Int(1), ParameterType::Integer);

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testSelectWithCTEAndCreateNamedParametersForEachQuery() {
    assert!(platform_supports_ctes() && platform_supports_cte_columns_definition());
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("virtual_id", 1)])]);
    let mut qb = connection.create_query_builder();

    let id1 = qb.create_named_parameter(Value::Int(1), ParameterType::Integer, Some(":id1"));
    let cte_query_builder1 = sub_select_virtual_id(&qb, &qb.expr().eq("id", &id1));

    let id2 = qb.create_named_parameter(one_two(), ArrayParameterType::Integer, Some(":id2"));
    let cte_query_builder2 = sub_select_virtual_id(&qb, &qb.expr().r#in("id", &[id2]));

    with_three_ctes(&mut qb, cte_query_builder1, cte_query_builder2);
    qb.set_parameter(0, Value::Int(1), ParameterType::Integer);

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testSelectWithCTEAndCreatePositionalParametersForEachQuery() {
    assert!(platform_supports_ctes() && platform_supports_cte_columns_definition());
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("virtual_id", 1)])]);
    let mut qb = connection.create_query_builder();

    let first = qb.create_positional_parameter(Value::Int(1), ParameterType::Integer);
    let cte_query_builder1 = sub_select_virtual_id(&qb, &qb.expr().eq("id", &first));

    let second = qb.create_positional_parameter(one_two(), ArrayParameterType::Integer);
    let cte_query_builder2 = sub_select_virtual_id(&qb, &qb.expr().r#in("id", &[second]));

    with_three_ctes(&mut qb, cte_query_builder1, cte_query_builder2);
    qb.set_parameter(2, Value::Int(1), ParameterType::Integer);

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testSelectWithCTEUnion() {
    assert!(platform_supports_ctes());
    let connection = set_up().await;
    let expected_rows = prepare_expected_rows(vec![row(&[("id", 2)]), row(&[("id", 1)])]);
    let mut qb = connection.create_query_builder();

    let sub_query_builder1 = sub_select_id(&qb, &qb.expr().eq("id", "?"));
    let sub_query_builder2 = sub_select_id(&qb, &qb.expr().eq("id", "?"));

    let mut sub_query_builder3 = qb.sub();
    sub_query_builder3
        .union(sub_query_builder1)
        .add_union(sub_query_builder2, UnionType::default())
        .unwrap();

    qb.with("cte_a", sub_query_builder3, None)
        .unwrap()
        .select(&["id"])
        .from("cte_a", None)
        .order_by("id", Some("DESC"))
        .set_parameters(
            PhpArray::list([Value::Int(1), Value::Int(2)]),
            [
                (0.into(), ParameterType::Integer.into()),
                (1.into(), ParameterType::Integer.into()),
            ],
        );

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}

#[tokio::test]
async fn testCTEUnionMainQuery() {
    assert!(platform_supports_ctes());
    let connection = set_up().await;
    let mut expected = Vec::new();
    for (id, value, sort) in [(2, "first", 1), (1, "second", 2)] {
        let mut row = PhpArray::new();
        row.insert("id", Value::Int(id));
        row.insert("value", Value::string(value));
        row.insert("sort", Value::Int(sort));
        expected.push(row);
    }
    let expected_rows = prepare_expected_rows(expected);
    let mut qb = connection.create_query_builder();

    let mut base_query_builder = qb.sub();
    base_query_builder.select(&["id"]).from("for_update", None);

    let mut union_part1 = qb.sub();
    union_part1
        .select(&[
            "id",
            &format!("{} AS value", qb.expr().literal("first").unwrap()),
            "1 AS sort",
        ])
        .from("cte_base", None)
        .r#where(qb.expr().eq("id", "2"), []);

    let mut union_part2 = qb.sub();
    union_part2
        .select(&[
            "id",
            &format!("{} AS value", qb.expr().literal("second").unwrap()),
            "2 AS sort",
        ])
        .from("cte_base", None)
        .r#where(qb.expr().eq("id", "1"), []);

    qb.with("cte_base", base_query_builder, None)
        .unwrap()
        .union(union_part1)
        .add_union(union_part2, UnionType::default())
        .unwrap()
        .order_by("sort", None);

    assert_eq!(expected_rows, fetch_all_associative(&qb).await);
}
