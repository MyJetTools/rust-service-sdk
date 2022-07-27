pub const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub trait GetGlobalState {
    fn is_initialized(&self) -> bool;

    fn is_shutting_down(&self) -> bool;

    fn shutting_down(&self);
}

pub trait GetLogStashUrl {
    fn get_logstash_url(&self) -> String;
}