pub type EntityID = u64;
type LookupID = u128;

pub mod manager;
pub mod system;
pub mod errors;
pub mod component;
mod communicator;
