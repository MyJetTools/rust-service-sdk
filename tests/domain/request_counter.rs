use tokio::sync::RwLock;

use super::database::Database;

pub struct RequestCounter {
    pub counter: i64
}

pub struct DatabaseImpl {
    counter: RwLock<RequestCounter>
}

impl DatabaseImpl {
    pub fn new() -> Self {
        DatabaseImpl {counter: RwLock::new(RequestCounter{counter:0})}
    }
}

#[async_trait::async_trait]
impl Database<RequestCounter> for DatabaseImpl {
    async fn read(&self) -> RequestCounter{
       let counter = self.counter.read().await;
       
       RequestCounter { counter:counter.counter}
    }

    async fn increase(&self) {
        let mut counter = self.counter.write().await;
        counter.counter +=1;
    }
}