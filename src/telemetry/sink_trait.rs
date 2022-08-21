use async_trait::async_trait;

#[async_trait]
pub trait AllSinkTrait : FinalizeLogs + CreateWriter {
    
}

#[async_trait]
pub trait FinalizeLogs {
    async fn finalize_logs(&self);
}

pub trait CreateWriter {
    fn create_writer(&self) -> Box<dyn std::io::Write + 'static>;
}