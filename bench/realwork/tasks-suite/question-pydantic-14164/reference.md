The PR as its author described it (https://github.com/pydantic/pydantic/pull/14164):
Fixes #9545.

When validating or serializing deeply nested (but acyclic) data, hitting the recursion depth limit was reported as a cyclic reference. The depth limit is unchanged, only the error is:

```python
class Model(BaseModel):
    x: 'Model | None' = None

data = {}
for _ in range(1_000):
    data = {'x': data}

Model.model_validate(data)
# Before: Recursion error - cyclic reference detected [type=recursion_loop, ...]
# After:  Recursion error - maximum validation depth exceeded [type=recursion_depth_exceeded, ...]
```

During serialization, the `ValueError` message changes from `Circular reference detected (depth exceeded)` to `Maximum serialization depth exceeded`.

Actual cycles still raise `recursion_loop` / `Circular reference detected (id repeated)`.

Its changes (non-test files):
```diff

```
A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), and names a real risk or says plainly that it sees none.
