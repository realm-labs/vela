use crate::stdlib::stdlib_function_fact;
use crate::type_fact::TypeFact;

#[test]
fn numeric_conversion_facts_preserve_exact_scalar_types() {
    let widening = stdlib_function_fact("i32::from_i16", &[TypeFact::I16])
        .expect("i32 widening conversion fact");
    assert_eq!(widening.returns, TypeFact::I32);

    let narrowing = stdlib_function_fact("i32::try_from_i64", &[TypeFact::I64])
        .expect("i32 checked narrowing conversion fact");
    assert_eq!(
        narrowing.returns,
        TypeFact::result(TypeFact::I32, TypeFact::STRING),
    );

    let narrow_u16 = stdlib_function_fact("u16::try_from_u64", &[TypeFact::U64])
        .expect("u16 checked narrowing conversion fact");
    assert_eq!(
        narrow_u16.returns,
        TypeFact::result(TypeFact::U16, TypeFact::STRING),
    );
}
