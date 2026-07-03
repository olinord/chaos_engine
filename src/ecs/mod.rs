pub type EntityID = u64;
type LookupID = u128;

pub mod component;
pub mod componentstore;
pub mod entity;
pub mod errors;
pub mod query;
pub mod system;
pub mod world;
