/// Snapshot of the state
/// It can give a value that has been written/created on given state
/// It should not query parents or database
/// [`ForkTreeManager`] suppose to operate over those
pub trait Snapshot {
    type Id: Clone;
    type Key;
    type Value: Clone;

    /// Get own value, value from its own cache
    fn get_value(&self, key: &Self::Key) -> Option<Self::Value>;

    /// Helper method for mapping
    fn get_id(&self) -> &Self::Id;
}