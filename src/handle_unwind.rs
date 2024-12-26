use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

/// Runs `try_fn`. If it unwinds, it will run `catch` and then continue unwinding
pub fn handle_unwind<R, F: FnOnce() -> R, G: FnOnce()>(try_fn: F, catch: G) -> R {
	let try_fn = AssertUnwindSafe(try_fn);
	catch_unwind(try_fn).unwrap_or_else(|e| {
		catch();
		resume_unwind(e)
	})
}
