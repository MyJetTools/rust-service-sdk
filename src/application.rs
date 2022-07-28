use std::{sync::Arc, future::Future};

use serde::de::DeserializeOwned;
use tokio_util::sync::CancellationToken;

use crate::{
    app::app_ctx::{GetGlobalState, GetLogStashUrl},
    configuration::{EnvConfig, SettingsReader},
    server,
    telemetry::{get_subscriber, init_subscriber, ElasticSink},
};

pub struct Application<TAppContext, TSettingsModel> {
    pub settings: Arc<TSettingsModel>,
    pub context: Arc<TAppContext>,
    pub env_config: Arc<EnvConfig>,
}

impl<TAppContext, TSettingsModel> Application<TAppContext, TSettingsModel>
where
    TAppContext: GetGlobalState + Send + Sync,
    TSettingsModel: DeserializeOwned + GetLogStashUrl,
{
    pub async fn init<TGetConext>(create_context: TGetConext) -> Self
    where
        TGetConext: Fn(&TSettingsModel) -> TAppContext,
    {
        let settings = SettingsReader::read_settings::<TSettingsModel>()
            .await
            .expect("Can't get settings!");

        let env_config = Arc::new(SettingsReader::read_env_settings());
        let context = Arc::new(create_context(&settings));

        Application {
            context,
            env_config,
            settings: Arc::new(settings),
        }
    }

    pub fn start_logger(&self) -> Arc<ElasticSink> {
        let sink = Arc::new(ElasticSink::new(
            self.settings.get_logstash_url().parse().unwrap(),
        ));
        let clone = sink.clone();
        let subscriber = get_subscriber(
            "rust_service_template".into(),
            "info".into(),
            move || clone.create_writer(),
            self.env_config.index.clone(),
            self.env_config.environment.clone(),
        );
        init_subscriber(subscriber);
        sink
    }

    pub async fn start_hosting<Func>(
        &self,
        register_services: Func,
    ) -> (
        tokio::task::JoinHandle<Result<(), anyhow::Error>>,
        tokio::task::JoinHandle<Result<(), anyhow::Error>>,
    )
    where
        Func: Fn(
                Box<std::cell::RefCell<tonic::transport::Server>>,
            ) -> tonic::transport::server::Router
            + Send
            + Sync
            + 'static,
    {
        let grpc_server = tokio::spawn(server::run_grpc_server(
            self.env_config.clone(),
            register_services,
        ));
        let http_server = tokio::spawn(server::run_http_server(self.env_config.clone()));

        (grpc_server, http_server)
    }

    pub async fn wait_for_termination<Func, Fut>(
        &self,
        sink: Arc<ElasticSink>,
        grpc_server: tokio::task::JoinHandle<Result<(), anyhow::Error>>,
        http_server: tokio::task::JoinHandle<Result<(), anyhow::Error>>,
        tasks_to_abort: &mut Vec<tokio::task::JoinHandle<Result<(), anyhow::Error>>>,
        cancellation: Option<Arc<CancellationToken>>,
        graceful_shutdown_task: Func,
        shutdown_time_ms: u64,
    ) where
            Func: FnOnce(Arc<TAppContext>) -> Fut,
            Fut: Future<Output = bool>, {
        let cancellation_token: CancellationToken;
        if let Some(parent_token) = cancellation {
            cancellation_token = parent_token.child_token();
        } else {
            cancellation_token = tokio_util::sync::CancellationToken::new();
        }
        
        let shut_func = || {
            self.context.shutting_down();
            cancellation_token.cancel();
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Stop signal received!");
                shut_func();
            },
            _ = cancellation_token.cancelled() => {
                tracing::info!("Stop signal received via cancellation token!");
                shut_func();
            },
            o = grpc_server => {
                report_exit("GRPC_SERVER", o);
                shut_func();
            }
            o = http_server => {
                report_exit("HTTP_SERVER", o);
                shut_func();
            }
        };
        
        // This is how shut down tasks
        while let Some(task) = tasks_to_abort.pop(){
            tokio::select! {
                _ = task => {},
                _ = cancellation_token.cancelled() => {},
            };
        }

        tokio::select! {
            _ = graceful_shutdown_task(self.context.clone()) => {
                tracing::info!("Graceful shutdown has finished!");
            },
            _ = tokio::time::sleep(std::time::Duration::from_millis(shutdown_time_ms)) => {
                tracing::info!("Graceful shutdown cancelled after waiting {} ms!", shutdown_time_ms);
            },
        };

        sink.finalize_logs().await;
    }
}

fn report_exit(
    task_name: &str,
    outcome: Result<Result<(), impl std::fmt::Debug + std::fmt::Display>, tokio::task::JoinError>,
) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed",
                task_name
            )
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{}' task failed to complete",
                task_name
            )
        }
    }
}
