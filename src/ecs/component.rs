use std::any::Any;

pub trait Component: Any {}

impl<T: Any> Component for T {}
