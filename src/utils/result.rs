pub fn unwrap_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>, message: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) => panic!("{}: {:?}", message, e),
    }
}
