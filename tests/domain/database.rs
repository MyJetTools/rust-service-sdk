use async_trait::async_trait;

#[async_trait]
pub trait Database<T> {
    async fn read(&self) -> T;
    async fn increase(&self);
}
