//! Compile-fail contracts of the derive layer (run by trybuild).

#[test]
fn ui_contracts() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
