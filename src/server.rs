use std::{sync::Arc, cell::RefCell};
use tonic::transport::{server::Router, Server};
use uuid::Uuid;

use crate::{
    app::{self},
    configuration::EnvConfig,
};

pub async fn run_grpc_server<Func>(
    env_config: Arc<EnvConfig>,
    register_services: Func,
) -> Result<(), anyhow::Error>
where
    Func: Fn(Box<RefCell<Server>>) -> Router,
{
    let addr = format!("{}:{}", env_config.base_url, env_config.grpc_port)
        .parse()
        .unwrap();

    println!("GRPC server listening on {}", addr);
    let builder = tonic::transport::Server::builder().trace_fn(|req| {
        tracing::info_span!(
            "grpc_call",
            grpc_request = format!("{:?}", req),
            trace_id = format!("{}", Uuid::new_v4())
        )
    });

    let wrapped = Box::new(RefCell::new(builder));

    let router = register_services(wrapped);

    router.serve(addr).await.unwrap();

    Ok(())
}

async fn handle(
    _: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, std::convert::Infallible> {
    let response = IsAlive {
        version: app::APP_VERSION.to_string(),
    };
    let serialized = serde_json::to_string(&response).unwrap();
    Ok(hyper::Response::new(serialized.into()))
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct IsAlive {
    #[serde(rename = "version")]
    version: String,
}

pub async fn run_http_server(env_config: Arc<EnvConfig>) -> Result<(), anyhow::Error> {
    let addr = format!("{}:{}", env_config.base_url, env_config.http_port)
        .parse()
        .unwrap();
    println!("HTTP server listening on {}", addr);
    let make_svc = hyper::service::make_service_fn(|_conn| async {
        Ok::<_, std::convert::Infallible>(hyper::service::service_fn(handle))
    });

    let server = hyper::Server::bind(&addr).serve(make_svc);

    server.await?;

    Ok(())
}
