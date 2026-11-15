The PR as its author described it (https://github.com/pydantic/pydantic/pull/14125):
Stacked on #14123.

Stream one level of simple nested models, including models inside lists and dictionaries. Deeper model nesting and unsupported validators continue to use ordinary validation.

Nested model fields are held until the complete JSON document has been checked, then model instances are constructed. This lets valid nested documents complete streaming while keeping storage descriptors from running on malformed JSON.

Matched local release measurements for a model containing 100 simple nested models (base → this PR):

| Input | Base | This PR |
| --- | ---: | ---: |
| Valid | 29.3 µs | 23.0 µs |
| Late field type error | 29.4 µs | 46.9 µs |
| Trailing invalid JSON | 9.7 µs | 26.1 µs |

The valid case is about 21% faster. Invalid inputs can cost more because streaming does partial work before falling back to ordinary parsing and error reporting. These are local walltime measurements; CodSpeed provides the CI comparison.

Validation: full pydantic-core suite (6,271 passed, 126 skipped, 8 xfailed), including 230 focused streaming tests; Rust library/test Clippy, Rust formatting, and Python lint/format checks pass. Added nested storage-descriptor regression coverage.

Its changes (non-test files):
```diff
diff --git a/pydantic-core/src/validators/mod.rs b/pydantic-core/src/validators/mod.rs
index d746ad941f5..13b5b0efaa8 100644
--- a/pydantic-core/src/validators/mod.rs
+++ b/pydantic-core/src/validators/mod.rs
@@ -146,7 +146,9 @@ impl SchemaValidator {
     /// differential test tell "both paths agree" apart from "only one path ran".
     #[getter]
     fn _stream_plan_accepted(&self, py: Python<'_>) -> bool {
-        self.stream_plan.is_some() && model_fields::root_parts(py, &self.validator).is_some()
+        self.stream_plan
+            .as_deref()
+            .is_some_and(|plan| model_fields::root_parts(py, &self.validator, plan).is_some())
     }
 
     #[new]
@@ -585,7 +587,7 @@ impl SchemaValidator {
         }
 
         let plan = self.stream_plan.as_deref()?;
-        let (class, fields) = model_fields::root_parts(py, &self.validator)?;
+        let (class, fields) = model_fields::root_parts(py, &self.validator, plan)?;
 
         let mut recursion_guard = RecursionState::default();
         let mut state = ValidationState::new(
diff --git a/pydantic-core/src/validators/model.rs b/pydantic-core/src/validators/model.rs
index d13aa8fc739..d69c83adfd8 100644
--- a/pydantic-core/src/validators/model.rs
+++ b/pydantic-core/src/validators/model.rs
@@ -66,6 +66,11 @@ pub struct ModelValidator {
 }
 
 impl ModelValidator {
+    /// Class already checked before parsing a streamed document.
+    pub(crate) fn stream_class(&self) -> &Py<PyType> {
+        &self.class
+    }
+
     /// The fixed schema requirements for streaming, independent of mutable class hooks.
     pub(crate) fn stream_fields(&self) -> Option<&super::model_fields::ModelFieldsValidator> {
         if self.custom_init || self.root_model || self.post_init.is_some() || self.frozen {
diff --git a/pydantic-core/src/validators/model_fields.rs b/pydantic-core/src/validators/model_fields.rs
index 0e7e9338038..ff80f9fff43 100644
--- a/pydantic-core/src/validators/model_fields.rs
+++ b/pydantic-core/src/validators/model_fields.rs
@@ -12,7 +12,7 @@ use pyo3::exceptions::{PyKeyError, PyTypeError};
 use pyo3::intern;
 use pyo3::prelude::*;
 use pyo3::pybacked::PyBackedStr;
-use pyo3::types::{PyDict, PyList, PySet, PyString, PyType};
+use pyo3::types::{PyDict, PyList, PySet, PyString, PyTuple, PyType};
 use smallvec::SmallVec;
 
 use crate::build_tools::py_schema_err;
@@ -306,6 +306,14 @@ pub(crate) enum Fast {
     List(Box<ContainerPlan>),
     /// an object with string keys whose values all come off the cursor the same way
     Dict(Box<ContainerPlan>),
+    Model(Box<NestedModelPlan>),
+}
+
+/// Owned instructions for a nested model, kept behind one pointer so field plans stay small.
+#[derive(Debug)]
+pub(crate) struct NestedModelPlan {
+    validator: Arc<CombinedValidator>,
+    plan: ModelPlan,
 }
 
 /// One field's plan: how to take its value, and whether null is also an answer for it.
@@ -370,17 +378,26 @@ pub(crate) fn root_plan(validator: &CombinedValidator) -> Option<ModelPlan> {
     let CombinedValidator::Model(model) = unwrap_prebuilt(validator) else {
         return None;
     };
-    model.stream_fields()?.stream_plan()
+    model.stream_fields()?.stream_plan(0)
 }
 
 /// Class hooks can change after schema construction, so check them for each call.
 pub(crate) fn root_parts<'v>(
     py: Python<'_>,
     validator: &'v CombinedValidator,
+    plan: &ModelPlan,
 ) -> Option<(&'v Py<PyType>, &'v ModelFieldsValidator)> {
     let CombinedValidator::Model(model) = unwrap_prebuilt(validator) else {
         return None;
     };
+    for node in &plan.nodes {
+        if let Fast::Model(nested) = &node.fast {
+            let CombinedValidator::Model(nested) = unwrap_prebuilt(&nested.validator) else {
+                unreachable!("a model plan holds a model validator");
+            };
+            nested.stream_parts(py)?;
+        }
+    }
     model.stream_parts(py)
 }
 
@@ -409,6 +426,15 @@ pub(crate) fn take_root<'py>(
         return Ok(None);
     };
     jiter.finish()?;
+    for (field, &index) in std::iter::zip(&fields.fields, &plan.fields) {
+        if !contains_nested_model(plan, index) {
+            continue;
+        }
+        let value = model_dict.get_item(&field.name)?.expect("streamed field is present");
+        if let Some(value) = materialize_nested(py, value, plan, index)? {
+            model_dict.set_item(&field.name, value)?;
+        }
+    }
     let instance = create_class(class.bind(py))?;
     let none = py.None();
     let fields_set = Bound::new(py, fields_set)?;
@@ -416,6 +442,62 @@ pub(crate) fn take_root<'py>(
     Ok(Some(instance))
 }
 
+fn contains_nested_model(plan: &ModelPlan, mut index: usize) -> bool {
+    loop {
+        match &plan.nodes[index].fast {
+            Fast::Model(_) => return true,
+            Fast::List(container) | Fast::Dict(container) => index = container.element,
+            _ => return false,
+        }
+    }
+}
+
+/// Replace the temporary fields of a nested model with its instance after JSON parsing succeeds.
+fn materialize_nested<'py>(
+    py: Python<'py>,
+    value: Bound<'py, PyAny>,
+    model_plan: &ModelPlan,
+    index: usize,
+) -> PyResult<Option<Bound<'py, PyAny>>> {
+    let field = &model_plan.nodes[index];
+    if field.nullable && pyo3::types::PyAnyMethods::is_none(&value) {
+        return Ok(None);
+    }
+    match &field.fast {
+        Fast::Model(nested) => {
+            let CombinedValidator::Model(nested) = unwrap_prebuilt(&nested.validator) else {
+                unreachable!("a model plan holds a model validator");
+            };
+            let class = nested.stream_class();
+            let pending = value.cast::<PyTuple>()?;
+            let model_dict = pending.get_item(0)?;
+            let fields_set = pending.get_item(1)?;
+            let instance = create_class(class.bind(py))?;
+            set_model_attrs(&instance, &model_dict, py.None().bind(py), &fields_set)?;
+            Ok(Some(instance))
+        
[diff truncated]
```
A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), and names a real risk or says plainly that it sees none.
