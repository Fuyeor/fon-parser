// fon-parser/tests/serde.rs

#[cfg(feature = "serde")]
#[test]
fn public_trees_implement_serde_traits() {
    fn assert_serde<T: serde::Serialize + serde::de::DeserializeOwned>() {}

    assert_serde::<fon_parser::Document>();
    assert_serde::<fon_parser::SyntaxTree>();
    assert_serde::<fon_parser::TypedDocument>();
    assert_serde::<fon_parser::Diagnostic>();
}
