The maintainers fixed it in https://github.com/pydantic/pydantic/pull/14051 ("Fix pickling of `PydanticCustomError` and `ValidationError`s containing custom errors").

Their description:
Fixes https://github.com/pydantic/pydantic/issues/14034.

<!-- Thank you for your contribution! -->
<!-- Unless your change is trivial, please create an issue to discuss the change before creating a PR -->

## Change Summary

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
diff --git a/pydantic-core/src/errors/validation_exception.rs b/pydantic-core/src/errors/validation_exception.rs
index f8e54eeef35..f332fa56e32 100644
--- a/pydantic-core/src/errors/validation_exception.rs
+++ b/pydantic-core/src/errors/validation_exception.rs
@@ -399,13 +399,28 @@ impl ValidationError {
         let py = slf.py();
         let callable = slf.getattr("from_exception_data")?;
         let borrow = slf.try_borrow()?;
-        let args = (
-            &borrow.title,
-            borrow.errors(py, include_url_env(py), true, true)?,
-            borrow.input_type,
-            borrow.hide_input,
-        )
-            .into_pyobject(py)?;
+        let errors = borrow.errors(py, include_url_env(py), true, true)?;
+        // Custom error types can't be looked up from their type string when unpickling,
+        // so pass the `PydanticCustomError` itself:
+        for (line_error, error) in borrow.line_errors.iter().zip(errors.bind(py).iter()) {
+            if let ErrorType::CustomError {
+                error_type,
+                message_template,
+                context,
+            } = &line_error.error_type
+            {
+                let custom_error = Bound::new(
+                    py,
+                    PydanticCustomError::py_new(
+                        error_type.clone(),
+                        message_template.clone(),
+                        context.as_ref().map(|c| c.bind(py).clone()),
+                    ),
+                )?;
+                error.cast::<PyDict>()?.set_item(intern!(py, "type"), custom_error)?;
+            }
+        }
+        let args = (&borrow.title, errors, borrow.input_type, borrow.hide_input).into_pyobject(py)?;
         Ok((callable, args))
     }
 }
diff --git a/pydantic-core/src/errors/value_exception.rs b/pydantic-core/src/errors/value_exception.rs
index e648e0acae6..1ead2270458 100644
--- a/pydantic-core/src/errors/value_exception.rs
+++ b/pydantic-core/src/errors/value_exception.rs
@@ -1,6 +1,7 @@
 use pyo3::exceptions::{PyException, PyValueError};
+use pyo3::intern;
 use pyo3::prelude::*;
-use pyo3::types::{PyDict, PyString};
+use pyo3::types::{PyDict, PyString, PyTuple};
 
 use crate::input::InputType;
 
@@ -103,6 +104,29 @@ impl PydanticCustomError {
             None => Ok(format!("{msg} [type={}, context=None]", self.error_type)),
         }
     }
+
+    // Instances created from Rust don't go through `BaseException.__init__()` and have empty `args`,
+    // so `BaseException.__reduce__()` can't be relied on:
+    fn __reduce__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
+        let py = slf.py();
+        let args = slf.getattr(intern!(py, "args"))?.cast_into::<PyTuple>()?;
+        // Only use the stored values if `args` is empty, as `args` reflects the arguments
+        // passed to (potentially overridden) `__new__()` implementations of subclasses:
+        let args = if args.is_empty() {
+            let this = slf.get();
+            match &this.context {
+                Some(context) => (&this.error_type, &this.message_template, context).into_pyobject(py)?,
+                None => (&this.error_type, &this.message_template).into_pyobject(py)?,
+            }
+        } else {
+            args
+        };
+        match slf.getattr_opt(intern!(py, "__dict__"))? {
+            // e.g. attributes set on subclass instances, or exception notes:
+            Some(dict) if dict.is_truthy()? => (slf.get_type(), args, dict).into_pyobject(py),
+            _ => (slf.get_type(), args).into_pyobject(py),
+        }
+    }
 }
 
 impl PydanticCustomError {

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
