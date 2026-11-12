Known finding (a reviewer raised it; the follow-up PR #43641 fixed it): the failure handling for resumed executions is
wrong at both ends.
- A queued execution that hits a temporary error at startup (for example a database error) is marked `crashed` at
  once, with no retry, instead of being counted against the resume attempt limit and retried.
- A waiting execution that fails the same way on every poll (for example its run data is missing) is reloaded every
  minute forever: the attempt limit does not stop it, because a non-retryable error does not use up an attempt.
The fix lets the resume attempt limit govern both cases (a queued execution stays `new` and keeps its attempt; a
waiting one keeps its attempt only on a retryable error).
A review that names either half, with where it happens, has found it. Other valid findings count too.
