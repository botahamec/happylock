use std::marker::PhantomData;

use crate::{Keyable, ThreadKey};

use super::{Scope, ScopedJoinHandle};

pub fn scope<'env, F, T>(key: impl Keyable, f: F) -> T
where
	F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
{
	let scope = Scope(PhantomData);
	let t = f(&scope);
	drop(key);
	t
}

impl<'scope> Scope<'scope, '_> {
	#[allow(clippy::unused_self)]
	pub fn spawn<T: Send + 'scope>(
		&self,
		f: impl FnOnce(ThreadKey) -> T + Send + 'scope,
	) -> std::io::Result<ScopedJoinHandle<'scope, T>> {
		unsafe {
			// safety: the lifetimes ensure that the data lives long enough
			let handle = std::thread::Builder::new().spawn_unchecked(|| {
				// safety: the thread just started, so the key cannot be acquired yet
				let key = ThreadKey::get().unwrap_unchecked();
				f(key)
			})?;

			Ok(ScopedJoinHandle {
				handle,
				_phantom: PhantomData,
			})
		}
	}
}

impl<T> ScopedJoinHandle<'_, T> {
	pub fn is_finished(&self) -> bool {
		self.handle.is_finished()
	}

	pub fn join(self) -> std::thread::Result<T> {
		self.handle.join()
	}
}
