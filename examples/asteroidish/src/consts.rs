#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceEvent {
    Resized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecializedEntities {
    Camera,
    Ship,
}
