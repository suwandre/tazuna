The maintainers fixed it in https://github.com/pydantic/pydantic/pull/13953 ("Fix JSON Schema for `arguments` schema").

Their description:
<!-- Thank you for your contribution! -->
<!-- Unless your change is trivial, please create an issue to discuss the change before creating a PR -->

## Change Summary

Fixes https://github.com/pydantic/pydantic/issues/13938.

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
diff --git a/pydantic/json_schema.py b/pydantic/json_schema.py
index 48e804165fb..b0a59dea118 100644
--- a/pydantic/json_schema.py
+++ b/pydantic/json_schema.py
@@ -2058,7 +2058,11 @@ def arguments_schema(self, schema: core_schema.ArgumentsSchema) -> JsonSchemaVal
 
         keyword_possible = not p_only_arguments and not var_args_schema
         if keyword_possible:
-            return self.kw_arguments_schema(kw_or_p_arguments + kw_only_arguments, var_kwargs_schema)
+            return self.kw_arguments_schema(
+                kw_or_p_arguments + kw_only_arguments,
+                var_kwargs_schema,
+                var_kwargs_mode=schema.get('var_kwargs_mode') or 'uniform',
+            )
 
         if not prefer_positional:
             positional_possible = not kw_only_arguments and not var_kwargs_schema
@@ -2070,12 +2074,18 @@ def arguments_schema(self, schema: core_schema.ArgumentsSchema) -> JsonSchemaVal
         )
 
     def kw_arguments_schema(
-        self, arguments: list[core_schema.ArgumentsParameter], var_kwargs_schema: CoreSchema | None
+        self,
+        arguments: list[core_schema.ArgumentsParameter],
+        var_kwargs_schema: CoreSchema | None,
+        *,
+        var_kwargs_mode: core_schema.VarKwargsMode = 'uniform',
     ) -> JsonSchemaValue:
         """Generates a JSON schema that matches a schema that defines a function's keyword arguments.
 
         Args:
             arguments: The core schema.
+            var_kwargs_schema: The core schema of the variadic keyword arguments, if any.
+            var_kwargs_mode: How the variadic keyword arguments are validated.
 
         Returns:
             The generated JSON schema.
@@ -2099,12 +2109,25 @@ def kw_arguments_schema(
         if required:
             json_schema['required'] = required
 
-        if var_kwargs_schema:
+        if var_kwargs_schema is None:
+            json_schema['additionalProperties'] = False
+        elif var_kwargs_mode == 'unpacked-typed-dict':
+            # The validator validates the extra keyword arguments against the typed dictionary,
+            # so its properties are merged with the arguments ones (overlapping names are rejected
+            # at schema generation time). The typed dictionary also controls extra keyword arguments.
+            # Note: in theory, `resolve_ref_schema()` could fail, but in practice this can't happen
+            # as the TypedDict's definition is generated before the arguments schema.
+            typed_dict_json_schema = self.resolve_ref_schema(self.generate_inner(var_kwargs_schema))
+            properties.update(typed_dict_json_schema.get('properties', {}))
+            required.extend(typed_dict_json_schema.get('required', []))
+            if required:
+                json_schema['required'] = required
+            if 'additionalProperties' in typed_dict_json_schema:
+                json_schema['additionalProperties'] = typed_dict_json_schema['additionalProperties']
+        else:
             additional_properties_schema = self.generate_inner(var_kwargs_schema)
             if additional_properties_schema:
                 json_schema['additionalProperties'] = additional_properties_schema
-        else:
-            json_schema['additionalProperties'] = False
         return json_schema
 
     def p_arguments_schema(
@@ -2193,7 +2216,7 @@ def arguments_v3_schema(self, schema: core_schema.ArgumentsV3Schema) -> JsonSche
             properties[name] = argument_schema
 
             if (
-                (mode == 'var_kwargs_unpacked_typed_dict' and 'required' in argument_schema)
+                (mode == 'var_kwargs_unpacked_typed_dict' and 'required' in self.resolve_ref_schema(argument_schema))
                 or mode not in {'var_args', 'var_kwargs_uniform', 'var_kwargs_unpacked_typed_dict'}
                 and argument['schema']['type'] != 'default'
             ):

```
A good answer includes this approach or an equally sound one, says why, and weighs at least one alternative.
