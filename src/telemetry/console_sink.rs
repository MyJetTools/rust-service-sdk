use async_trait::async_trait;

use super::{sink_trait::FinalizeLogs, CreateWriter, AllSinkTrait};

pub struct ConsoleSink {
}

pub struct ConsoleWriter {
}

impl ConsoleSink {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl AllSinkTrait for ConsoleSink {
}


#[async_trait]
impl FinalizeLogs for ConsoleSink {
    async fn finalize_logs(&self) {

    }
}

impl CreateWriter for ConsoleSink {
    fn create_writer(&self) -> Box<(dyn std::io::Write + 'static)> {
        Box::new(ConsoleWriter {})
    }
}

impl std::io::Write for ConsoleWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::io::stdout().write_all(&buf).unwrap();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Result::Ok(())
    }
}
