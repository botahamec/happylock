use std::marker::PhantomData;

mod scope;

#[derive(Debug)]
pub struct Scope<'scope, 'env: 'scope>(PhantomData<(&'env (), &'scope ())>);

#[derive(Debug)]
pub struct ScopedJoinHandle<'scope, T> {
	handle: std::thread::JoinHandle<T>,
	_phantom: PhantomData<&'scope ()>,
}

pub struct JoinHandle<T> {
	handle: std::thread::JoinHandle<T>,
	key: crate::ThreadKey,
}

pub struct ThreadBuilder(std::thread::Builder);
