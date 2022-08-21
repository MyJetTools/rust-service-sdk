mod telemetry;
mod elastic_sink;
mod formatting_layer;
mod sink_trait;
mod console_sink;

pub use sink_trait::AllSinkTrait;
pub use sink_trait::FinalizeLogs;
pub use sink_trait::CreateWriter;
pub use telemetry::get_subscriber;
pub use telemetry::init_subscriber;

pub use elastic_sink::ElasticSink;
pub use elastic_sink::ElasticWriter;

pub use console_sink::ConsoleSink;
pub use console_sink::ConsoleWriter;

pub use formatting_layer::CustomFormattingLayer;
