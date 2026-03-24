pub trait OutputPort: Send + Sync {
    fn write(&self, output: &str);
}
