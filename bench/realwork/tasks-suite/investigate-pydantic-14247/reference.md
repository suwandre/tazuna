The maintainers fixed it in https://github.com/pydantic/pydantic/pull/14247 ("Bump jiter to v0.19.0").

Their description:
To address memory issues reported in https://github.com/pydantic/pydantic/issues/14234.

The fix (non-test files):
```diff
diff --git a/pydantic-core/Cargo.lock b/pydantic-core/Cargo.lock
index e77109a208..8d004c57ea 100644
--- a/pydantic-core/Cargo.lock
+++ b/pydantic-core/Cargo.lock
@@ -312,9 +312,9 @@ checksum = "49f1f14873335454500d59611f1cf4a4b0f786f9ac11f4312a78e4cf2566695b"
 
 [[package]]
 name = "jiter"
-version = "0.18.0"
+version = "0.19.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "49e31424b61d85292bd805dbf643c3ccc44f5f3871fb751209b889ee70585029"
+checksum = "4027f86130c7a1997f880ed407580e58514b867af49e7ce44370f80fa6ff999f"
 dependencies = [
  "ahash",
  "bitvec",
diff --git a/pydantic-core/Cargo.toml b/pydantic-core/Cargo.toml
index 9f292d8152..78b56f0d01 100644
--- a/pydantic-core/Cargo.toml
+++ b/pydantic-core/Cargo.toml
@@ -45,7 +45,7 @@ base64 = "0.23.1"
 num-bigint = "0.4.6"
 num-traits = "0.2.19"
 uuid = "1.23.2"
-jiter = { version = "0.18.0", features = ["python"] }
+jiter = { version = "0.19.0", features = ["python"] }
 hex = "0.4.3"
 percent-encoding = "2.3.2"
 hashbrown = { version = "0.17", default-features = false, features = ["inline-more"] }

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
