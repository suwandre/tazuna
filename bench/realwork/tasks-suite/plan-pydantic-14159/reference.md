The maintainers fixed it in https://github.com/pydantic/pydantic/pull/14159 ("Infer the type of bare `Final` fields from their default value").

Their description:
Fixes #11119, part of #10033 (follow-up of #14158, which made `Final` annotations with a default fields).

When `Final` is used without a type argument, the type of the field is now inferred from the default value, as type checkers do:

```python
class Model(BaseModel):
    a: Final = 1
    b: Final = Field(default_factory=list[int])

Model.model_fields['a'].annotation
#> Before: Any
#> After: int
Model.model_fields['b'].annotation
#> Before: Any
#> After: list[int]

Model(a='not an int')
#> Before: Model(a='not an int', b=[])
#> After: ValidationError
```

The type is inferred from the default value (or the default of `Field()`/`dataclasses.field()`), or from the `default_factory` if it is a class or a generic alias (e.g. `list[int]`). Otherwise, `Any` is used.

The immutability documentation now mentions `Final`.

The fix (non-test files):
```diff
diff --git a/docs/concepts/fields.md b/docs/concepts/fields.md
index 22e3f8d487c..40cab7c9f3c 100644
--- a/docs/concepts/fields.md
+++ b/docs/concepts/fields.md
@@ -653,6 +653,9 @@ except ValidationError as e:
 
 1. Since `name` field is frozen, the assignment is not allowed.
 
+Fields can also be made immutable using the [`Final`][typing.Final] type qualifier (e.g. `name: Final[str]`).
+If no type is provided (e.g. `name: Final = 'John'`), it is inferred from the default value.
+
 <!-- old anchor added for backwards compatibility -->
 <!-- markdownlint-disable-next-line no-empty-links -->
 [](){#exclude}
diff --git a/pydantic/fields.py b/pydantic/fields.py
index 4d07a776cb7..41c995b9bba 100644
--- a/pydantic/fields.py
+++ b/pydantic/fields.py
@@ -16,9 +16,23 @@
 
 import annotated_types
 from pydantic_core import MISSING, PydanticUndefined
-from typing_extensions import Self, TypedDict, TypeForm, Unpack, deprecated
+from typing_extensions import (  # noqa: UP035 (for `get_origin`)
+    Self,
+    TypedDict,
+    TypeForm,
+    Unpack,
+    deprecated,
+    get_origin,
+)
 from typing_inspection import typing_objects
-from typing_inspection.introspection import UNKNOWN, AnnotationSource, ForbiddenQualifier, Qualifier, inspect_annotation
+from typing_inspection.introspection import (
+    UNKNOWN,
+    AnnotationSource,
+    ForbiddenQualifier,
+    Qualifier,
+    inspect_annotation,
+    is_union_origin,
+)
 
 from . import types
 from ._internal import _decorators, _fields, _generics, _repr, _typing_extra, _utils
@@ -590,9 +604,14 @@ class MyModel(pydantic.BaseModel):
 
         # TODO check for classvar and error?
 
-        # TODO infer from the default (e.g. for `x: Final = 1`), as type checkers do:
-        type_expr: Any = Any if inspected_ann.type is UNKNOWN else inspected_ann.type
         final = 'final' in inspected_ann.qualifiers
+        if inspected_ann.type is not UNKNOWN:
+            type_expr: Any = inspected_ann.type
+        elif final:
+            # Bare `Final` qualifier (e.g. `x: Final = 1`), infer the type from the default as type checkers do:
+            type_expr = _infer_type_from_default(default)
+        else:
+            type_expr = Any
         metadata = inspected_ann.metadata
 
         # HACK 1: the order in which the metadata is merged is inconsistent; we need to prepend
@@ -971,6 +990,30 @@ def __repr_args__(self) -> ReprArgs:
                     yield s, value
 
 
+def _infer_type_from_default(default: Any) -> Any:
+    if isinstance(default, FieldInfo):
+        default_value, default_factory = default.default, default.default_factory
+    elif isinstance(default, dataclasses.Field):
+        default_value = PydanticUndefined if default.default is dataclasses.MISSING else default.default
+        default_factory = None if default.default_factory is dataclasses.MISSING else default.default_factory
+    else:
+        default_value, default_factory = default, None
+
+    if default_value is not PydanticUndefined and default_value is not Ellipsis:
+        return type(default_value)
+
+    origin = get_origin(default_factory)
+    if origin is not None and not (
+        typing_objects.is_annotated(origin) or typing_objects.is_literal(origin) or is_union_origin(origin)
+    ):
+        # e.g. `list[int]`:
+        return default_factory
+    if isinstance(default_factory, type):
+        # e.g. `list` or `dict`, creating an empty instance of the type:
+        return default_factory
+    return Any
+
+
 _Attrs = {
     'default': ...,
     'default_factory': None,

```
A good answer includes this approach or an equally sound one, says why, and weighs at least one alternative.
