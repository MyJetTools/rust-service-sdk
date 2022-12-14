use tracing::{trace, warn};

pub struct LoggerAdapter;

impl rust_extensions::Logger for LoggerAdapter {
    fn write_info(
        &self,
        process: String,
        message: String,
        _ctx: Option<std::collections::HashMap<String, String>>,
    ) {
        trace!("{}: {}", process, message);
    }

    fn write_warning(
        &self,
        process: String,
        message: String,
        _ctx: Option<std::collections::HashMap<String, String>>,
    ) {
        warn!("{}: {}", process, message)
    }

    fn write_error(
        &self,
        process: String,
        message: String,
        _ctx: Option<std::collections::HashMap<String, String>>,
    ) {
        tracing::error!("{}: {}", process, message);
    }

    fn write_fatal_error(
        &self,
        process: String,
        message: String,
        _ctx: Option<std::collections::HashMap<String, String>>,
    ) {
        tracing::error!("{}: {}", process, message);
    }
}
