Known gap (the follow-up PR #14159 closed it): with this PR, a `Final` annotation with a default becomes a field, but a
bare `Final` without a type argument (`a: Final = 1`) gets the annotation `Any`, so the field is not validated:
`Model(a='not an int')` is accepted. Type checkers infer the type from the default value; the follow-up does the same
(from the default, a `Field()` / `dataclasses.field()` default, or a class / generic-alias `default_factory`).
A review that notes bare `Final` ends up as `Any` / unvalidated, or asks what type such a field gets, has found it.
Other valid findings count too.
