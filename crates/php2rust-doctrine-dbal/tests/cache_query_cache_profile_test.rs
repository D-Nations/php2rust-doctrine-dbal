//! Port of tests/Cache/QueryCacheProfileTest.php.
//!
//! Upstream: doctrine/dbal tests/Cache/QueryCacheProfileTest.php
#![allow(non_snake_case)]

use php_value::vars::parse_str;
use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::cache::QueryCacheProfile;
use php2rust_doctrine_dbal::{ParameterType, ParameterTypes, WrapperParameterType};

const LIFETIME: i64 = 3600;
const CACHE_KEY: &str = "user_specified_cache_key";
const QUERY: &str = "SELECT * FROM foo WHERE bar = ?";

struct Fixture {
    query_cache_profile: QueryCacheProfile,
    params: PhpArray,
    types: ParameterTypes,
    connection_params: PhpArray,
}

fn pairs(pairs: &[(&str, &str)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(*key, Value::string(*value));
    }
    array
}

fn set_up() -> Fixture {
    Fixture {
        query_cache_profile: QueryCacheProfile::new(LIFETIME, Some(CACHE_KEY.to_owned()), None),
        params: PhpArray::list([Value::Int(666)]),
        types: [(
            Key::Int(0),
            WrapperParameterType::Parameter(ParameterType::Integer),
        )]
        .into_iter()
        .collect(),
        connection_params: pairs(&[
            ("dbname", "database_name"),
            ("user", "database_user"),
            ("password", "database_password"),
            ("host", "database_host"),
            ("driver", "database_driver"),
        ]),
    }
}

impl Fixture {
    fn generate(&self) -> (String, php_value::PhpString) {
        self.query_cache_profile.generate_cache_keys(
            QUERY,
            &self.params,
            &self.types,
            &self.connection_params,
        )
    }
}

#[test]
fn testShouldUseTheGivenCacheKeyIfPresent() {
    let (cache_key, _) = set_up().generate();

    assert_eq!(
        cache_key, CACHE_KEY,
        "The returned cache key should match the given one"
    );
}

#[test]
fn testShouldGenerateAnAutomaticKeyIfNoKeyHasBeenGiven() {
    let mut fixture = set_up();
    fixture.query_cache_profile = fixture.query_cache_profile.set_cache_key(None);

    let (cache_key, _) = fixture.generate();

    assert_ne!(
        cache_key, CACHE_KEY,
        "The returned cache key should be generated automatically"
    );

    assert!(
        !cache_key.is_empty(),
        "The generated cache key should not be empty"
    );
}

#[test]
fn testShouldGenerateDifferentKeysForSameQueryAndParamsAndDifferentConnections() {
    let mut fixture = set_up();
    fixture.query_cache_profile = fixture.query_cache_profile.set_cache_key(None);

    let (first_cache_key, _) = fixture.generate();

    fixture
        .connection_params
        .insert("host", Value::string("a_different_host"));

    let (second_cache_key, _) = fixture.generate();

    assert_ne!(
        first_cache_key, second_cache_key,
        "Cache keys should be different"
    );
}

#[test]
fn testConnectionParamsShouldBeHashed() {
    let mut fixture = set_up();
    fixture.query_cache_profile = fixture.query_cache_profile.set_cache_key(None);

    let (_, query_string) = fixture.generate();

    let params = parse_str(query_string.as_bytes());

    let connection_params = params.get(&Key::from("connectionParams"));
    assert!(connection_params.is_some());

    for (_, param) in fixture.connection_params.iter() {
        let Some(Value::String(hashed)) = connection_params else {
            panic!("connectionParams is not a string");
        };
        let Value::String(param) = param else {
            panic!("the parameters are strings");
        };
        assert!(
            !hashed
                .as_bytes()
                .windows(param.as_bytes().len())
                .any(|window| window == param.as_bytes())
        );
    }
}

#[test]
fn testShouldGenerateSameKeysIfNoneOfTheParamsChanges() {
    let mut fixture = set_up();
    fixture.query_cache_profile = fixture.query_cache_profile.set_cache_key(None);

    let (first_cache_key, _) = fixture.generate();

    let (second_cache_key, _) = fixture.generate();

    assert_eq!(
        first_cache_key, second_cache_key,
        "Cache keys should be the same"
    );
}

#[test]
fn testShouldGenerateDifferentPasswordInTheParams() {
    let fixture = set_up();
    let (_, first_real_cache_key) = fixture.query_cache_profile.generate_cache_keys(
        QUERY,
        &fixture.params,
        &fixture.types,
        &pairs(&[("user", "database_user"), ("password", "first-password")]),
    );

    let (_, second_real_cache_key) = fixture.query_cache_profile.generate_cache_keys(
        QUERY,
        &fixture.params,
        &fixture.types,
        &pairs(&[("user", "database_user"), ("password", "second-password")]),
    );

    assert_eq!(
        first_real_cache_key, second_real_cache_key,
        "Cache keys for different password should be the same"
    );
}

/// The keys as PHP 8.3 computes them (`serialize()`, `hash('sha256')`
/// and `sha1()` over the same inputs): the query, `serialize($params)`,
/// `serialize($types)` (an enum case) and the SHA-256 of the serialized
/// connection parameters without the password; the generated key is the
/// SHA-1 of the real one.
#[test]
fn real_key_matches_php() {
    let mut fixture = set_up();
    fixture.query_cache_profile = fixture.query_cache_profile.set_cache_key(None);
    let (cache_key, real) = fixture.generate();
    assert_eq!(
        real.as_bytes(),
        b"query=SELECT * FROM foo WHERE bar = ?&params=a:1:{i:0;i:666;}\
          &types=a:1:{i:0;E:35:\"Doctrine\\DBAL\\ParameterType:INTEGER\";}\
          &connectionParams=aea4e1ba47bcbd84c3b068e57c1a1e46db7fc2b1e93e47896f54994cce0c08ec"
            .as_slice(),
    );
    assert_eq!(cache_key, "3739aee614cf5b236d0cc096f17c0786c41f5973");
}
