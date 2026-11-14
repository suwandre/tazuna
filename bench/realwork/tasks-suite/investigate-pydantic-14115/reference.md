The maintainers fixed it in https://github.com/pydantic/pydantic/pull/14115 ("Fix `RecursionError` when copying or unpickling models with `defer_build`").

Their description:
Fixes https://github.com/pydantic/pydantic/issues/14089.

The fix (non-test files):
```diff
diff --git a/pydantic/_internal/_mock_val_ser.py b/pydantic/_internal/_mock_val_ser.py
index b0344ac0609..74c0856e038 100644
--- a/pydantic/_internal/_mock_val_ser.py
+++ b/pydantic/_internal/_mock_val_ser.py
@@ -95,6 +95,10 @@ def __init__(
 
     def __getattr__(self, item: str) -> None:
         __tracebackhide__ = True
+        # A slot name only reaches `__getattr__()` if it was never set, i.e. the instance was created without `__init__()`
+        # (when copying/unpickling).
+        if item in MockValSer.__slots__:
+            raise AttributeError(item)
         if self._attempt_rebuild:
             val_ser = self._attempt_rebuild()
             if val_ser is not None:

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
