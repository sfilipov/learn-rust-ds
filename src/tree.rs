pub trait TreeOps<T> {
    fn insert(&mut self, value: T) -> bool;
    fn contains(&self, value: &T) -> bool;
    fn remove(&mut self, value: &T) -> bool;
    fn len(&self) -> usize;
}
