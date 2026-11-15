The maintainers fixed it in https://github.com/pydantic/pydantic/pull/14052 ("Do not extract attribute docstrings from nested classes and functions").

Their description:
<!-- Thank you for your contribution! -->
<!-- Unless your change is trivial, please create an issue to discuss the change before creating a PR -->

## Change Summary

Fixes https://github.com/pydantic/pydantic/issues/14041.

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
diff --git a/pydantic/_internal/_docs_extraction.py b/pydantic/_internal/_docs_extraction.py
index 8808c1f08d3..99dc246acad 100644
--- a/pydantic/_internal/_docs_extraction.py
+++ b/pydantic/_internal/_docs_extraction.py
@@ -37,6 +37,16 @@ def visit_Expr(self, node: ast.Expr) -> Any:
                 self.attrs[self.target] = docstring
             self.target = None
 
+    # Don't look into nested classes and functions, as their attributes don't belong to the inspected class:
+    def visit_ClassDef(self, node: ast.ClassDef) -> Any:
+        pass
+
+    def visit_FunctionDef(self, node: ast.FunctionDef) -> Any:
+        pass
+
+    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> Any:
+        pass
+
 
 def _dedent_source_lines(source: list[str]) -> str:
     # Required for nested class definitions, e.g. in a function block
@@ -108,6 +118,11 @@ def extract_docstrings_from_cls(cls: type[Any], use_inspect: bool = False) -> di
 
     dedent_source = _dedent_source_lines(source)
 
+    stmt = ast.parse(dedent_source).body[0]
+    if isinstance(stmt, ast.FunctionDef) and stmt.name == 'dedent_workaround':
+        # `_dedent_source_lines()` wrapped the class around the workaround function
+        stmt = stmt.body[0]
+
     visitor = DocstringVisitor()
-    visitor.visit(ast.parse(dedent_source))
+    visitor.generic_visit(stmt)
     return visitor.attrs

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
