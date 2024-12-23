use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

pub fn handle_unwind<R, F: FnOnce() -> R, G: FnOnce()>(try_fn: F, catch: G) -> R {
	let try_fn = AssertUnwindSafe(try_fn);
	catch_unwind(try_fn).unwrap_or_else(|e| {
		catch();
		resume_unwind(e)
	})
}
