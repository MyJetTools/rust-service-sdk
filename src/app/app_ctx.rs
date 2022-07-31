pub const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub trait GetGlobalState {
    fn is_initialized(&self) -> bool;

    fn is_shutting_down(&self) -> bool;

    fn shutting_down(&self);
}

pub trait InitGrpc {
    fn init_grpc(
        &self,
        server: Box<std::cell::RefCell<tonic::transport::Server>>,
    ) -> tonic::transport::server::Router;
}

pub trait GetLogStashUrl {
    fn get_logstash_url(&self) -> String;
}
