The maintainers fixed it in https://github.com/pydantic/pydantic/pull/14054 ("Fix forward annotations evaluation and type variables substitution when rebuilding parametrized generic models").

Their description:
<!-- Thank you for your contribution! -->
<!-- Unless your change is trivial, please create an issue to discuss the change before creating a PR -->

## Change Summary

Alternative to https://github.com/pydantic/pydantic/pull/14053, which doesn't introduce regressions.

Fixes https://github.com/pydantic/pydantic/issues/14021.
Fixes https://github.com/pydantic/pydantic/issues/11223.

<!-- Please give a short summary of the changes. -->

## Related issue number

<!-- WARNING: please use "fix #123" style references so the issue is closed when this PR is merged. -->

## Checklist

* [ ] The pull request title is a good summary of the changes - it will be used in the changelog
* [ ] Unit tests for the changes exist
* [ ] Tests pass on CI
* [ ] Documentation reflects the changes where applicable
* [ ] My PR is ready to review, **please add a comment including the phrase "please review" to assign reviewers**

The fix (non-test files):
```diff
diff --git a/pydantic/_internal/_fields.py b/pydantic/_internal/_fields.py
index 51e7c9f0139..a8ce945c1f9 100644
--- a/pydantic/_internal/_fields.py
+++ b/pydantic/_internal/_fields.py
@@ -603,13 +603,36 @@ def rebuild_model_fields(
         schema generation, where you don't want to mutate other model's fields.
     """
     rebuilt_fields: dict[str, FieldInfo] = {}
+    origin = cls.__pydantic_generic_metadata__['origin']
     with ns_resolver.push(cls):
         for f_name, field_info in cls.__pydantic_fields__.items():
             if field_info._complete:
                 rebuilt_fields[f_name] = field_info
             else:
+                annotation = field_info._original_annotation
+                if origin is not None and (origin_field := origin.__pydantic_fields__.get(f_name)) is not None:
+                    # For parametrized generic models, the stored annotation can mix names from the origin class
+                    # and (forward) type arguments, which need to be evaluated in different namespaces. For instance:
+                    #
+                    # class Model[T](BaseModel):
+                    #     a: dict[T, 'Later']
+                    #
+                    # M = Model['Item']
+                    #
+                    # The stored annotation of `M.a` is `dict['Item', 'Later']`, where `'Later'` must be evaluated
+                    # in the namespace of `Model` and `'Item'` in the namespace of `M`. Instead,
+                    # first evaluate the (unsubstituted) annotation of the origin in its own namespace:
+                    with ns_resolver.push(origin):
+                        annotation = _typing_extra.eval_type(
+                            origin_field._original_annotation,
+                            *ns_resolver.types_namespace,
+                        )
                 new_field = _recreate_field_info(
-                    field_info, ns_resolver=ns_resolver, typevars_map=typevars_map, lenient=False
+                    field_info,
+                    ns_resolver=ns_resolver,
+                    typevars_map=typevars_map,
+                    lenient=False,
+                    annotation=annotation,
                 )
                 update_field_from_config(config_wrapper, f_name, new_field)
                 rebuilt_fields[f_name] = new_field
@@ -641,9 +664,13 @@ def _recreate_field_info(
     typevars_map: Mapping[TypeVar, Any],
     *,
     lenient: bool,
+    annotation: Any = PydanticUndefined,
 ) -> FieldInfo:
     FieldInfo_ = import_cached_field_info()
 
+    if annotation is PydanticUndefined:
+        annotation = field_info._original_annotation
+
     existing_desc = field_info.description
     if lenient:
         ann = _generics.replace_types(field_info._original_annotation, typevars_map)
@@ -655,7 +682,7 @@ def _recreate_field_info(
         # Not the best pattern, maybe we could ship our own `eval_type()`,
         # that would replace the type variables on the fly during evaluation.
         ann = _typing_extra.eval_type(
-            field_info._original_annotation,
+            annotation,
             *ns_resolver.types_namespace,
         )
         ann = _generics.replace_types(ann, typevars_map)
diff --git a/pydantic/_internal/_generate_schema.py b/pydantic/_internal/_generate_schema.py
index 0d03cd299e6..866db4e9a5a 100644
--- a/pydantic/_internal/_generate_schema.py
+++ b/pydantic/_internal/_generate_schema.py
@@ -109,7 +109,7 @@
     update_field_from_config,
 )
 from ._forward_ref import PydanticRecursiveRef
-from ._generics import get_standard_typevars_map, replace_types
+from ._generics import get_model_typevars_map, get_standard_typevars_map, replace_types
 from ._import_utils import import_cached_base_model, import_cached_field_info
 from ._mock_val_ser import MockCoreSchema
 from ._namespace_utils import NamespacesTuple, NsResolver
@@ -884,7 +884,7 @@ def _model_schema(self, cls: type[BaseModel]) -> core_schema.CoreSchema:
                             cls,
                             config_wrapper=self._config_wrapper,
                             ns_resolver=self._ns_resolver,
-                            typevars_map=self._typevars_map or {},
+                            typevars_map=get_model_typevars_map(cls),
                         )
                     except NameError as e:
                         raise PydanticUndefinedAnnotation.from_name_error(e) from e

```
A good answer includes this approach or an equally sound one, says why, and weighs at least one alternative.
