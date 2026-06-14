# Container Type Hint Contracts

> **Status:** superseded execution plan summary.

The original container type-hints implementation plan has moved to
[archive/container-type-hints-plan.md](archive/container-type-hints-plan.md).
It describes the older pre-value-keyed slice that restricted maps to string
keys and limited sets to a narrow set of leaf value contracts.

Current Vela container contracts are governed by:

- [architecture/primitives-type-hints-and-guards.md](architecture/primitives-type-hints-and-guards.md)
- [architecture/runtime-vm.md](architecture/runtime-vm.md)
- [value-keyed-map-set-plan.md](value-keyed-map-set-plan.md)

The current public contract is:

- `Array<T>` tracks and validates element contracts.
- `Map<K, V>` uses runtime `Value` keys. Key equality is defined by `ValueKey`,
  where immutable leaf values compare by value, script heap objects and host
  refs compare by identity, and unsupported transient values are rejected before
  mutation.
- `Set<T>` uses the same `ValueKey` policy for elements.
- `Iterator<T>`, `Option<T>`, and `Result<T, E>` preserve their typed contract
  behavior.

`ValueKey` is the only key-equality boundary for maps and sets. It must not
call user `PartialEq`, `Eq`, `PartialOrd`, `Ord`, or any future script-visible
`Hash` implementation.

