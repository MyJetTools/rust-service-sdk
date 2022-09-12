use std::{future::Future, sync::Arc};

use serde::de::DeserializeOwned;
use tokio_util::sync::CancellationToken;

use crate::{
    app::app_ctx::{GetGlobalState, GetLogStashUrl, InitGrpc},
    configuration::{EnvConfig, SettingsReader},
    server,
    telemetry::{get_subscriber, init_subscriber, ElasticSink, ConsoleSink, AllSinkTrait},
};

pub struct Application<TAppContext, TSettingsModel> {
    //pub life: &'a PhantomData<()>,
    pub settings: Arc<TSettingsModel>,
    pub context: Arc<TAppContext>,
    pub env_config: Arc<EnvConfig>,
    pub grpc_server: Box<tokio::task::JoinHandle<Result<(), anyhow::Error>>>,
    pub http_server: Box<tokio::task::JoinHandle<Result<(), anyhow::Error>>>,
}

impl<TAppContext, TSettingsModel> Application<TAppContext, TSettingsModel>
where
    TAppContext: GetGlobalState + InitGrpc + Send + Sync,
    TSettingsModel: DeserializeOwned + GetLogStashUrl + Clone,
{
    
    pub async fn init<'a, TGetConext, Fut>(create_context: TGetConext) -> Self
    where
        TGetConext: Send + Sync + 'static,
        TGetConext: Fn(TSettingsModel) -> Fut,
        Fut: Future<Output = TAppContext> + Send + Sync,
    {
        let settings = SettingsReader::read_settings::<TSettingsModel>()
            .await
            .expect("Can't get settings!");

        let env_config = Arc::new(SettingsReader::read_env_settings());
        let ctx = create_context(settings.clone()).await;
        let context = Arc::new(ctx);

        Application {
            //life: & PhantomData,
            context,
            env_config,
            settings: Arc::new(settings),
            grpc_server: Box::new(tokio::spawn(server::get_stab())),
            http_server: Box::new(tokio::spawn(server::get_stab())),
        }
    }

    fn start_logger(&self, app_name: String) -> Arc<dyn AllSinkTrait + Send + Sync + 'static> {
        let url = self.settings.get_logstash_url();
        let sink: Arc<dyn AllSinkTrait + Send + Sync>;
        if url.len() > 0 {
            println!("Using ELASTIC SINK!");
            sink = Arc::new(ElasticSink::new(
                url.parse().unwrap(),
            ));
            
        } else {
            println!("Using CONSOLE SINK ONLY!");
            sink = Arc::new(ConsoleSink::new());
        }

        let clone = sink.clone();

        let subscriber = get_subscriber(
            app_name,
            "info".into(),
            move || clone.create_writer(),
            self.env_config.index.clone(),
            self.env_config.environment.clone(),
        );
        init_subscriber(subscriber);
        sink
    }

    pub async fn start_hosting<Func>(&mut self, func: Func, app_name: String) -> Arc<dyn AllSinkTrait + Send + Sync>
    where
        Func: Fn(
                Box<std::cell::RefCell<tonic::transport::Server>>,
            ) -> tonic::transport::server::Router
            + Send
            + Sync
            + 'static,
    {
        let sink = self.start_logger(app_name);

        let grpc_server = tokio::spawn(server::run_grpc_server(self.env_config.clone(), func));
        let http_server = tokio::spawn(server::run_http_server(self.env_config.clone()));

        self.grpc_server = Box::new(grpc_server);
        self.http_server = Box::new(http_server);
        sink
    }

    pub async fn wait_for_termination<'a, Func, Fut>(
        &mut self,
        sink: Arc<dyn AllSinkTrait + Send + Sync + 'a>,
        tasks_to_abort: &mut Vec<tokio::task::JoinHandle<Result<(), anyhow::Error>>>,
        cancellation: Option<Arc<CancellationToken>>,
        graceful_shutdown_task: Func,
        shutdown_time_ms: u64,
    ) where
        Func: FnOnce(Arc<TAppContext>) -> Fut,
        Fut: Future<Output = bool>,
    {
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
            _ = wait_for_sigterm() => {
                tracing::info!("Termination signal received!");
                shut_func();
            },
            _ = cancellation_token.cancelled() => {
                tracing::info!("Stop signal received via cancellation token!");
                shut_func();
            },
            o = &mut self.grpc_server => {
                report_exit("GRPC_SERVER", o);
                shut_func();
            }
            o = &mut self.http_server => {
                report_exit("HTTP_SERVER", o);
                shut_func();
            }
        };

        // This is how shut down tasks
        while let Some(task) = tasks_to_abort.pop() {
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

#[cfg(not(target_os = "windows"))]
async fn wait_for_sigterm() -> Result<(), Box<dyn std::error::Error>> {
    
    let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    signal.recv().await;

    Ok(())
}

#[cfg(target_os = "windows")]
async fn wait_for_sigterm() -> Result<(), Box<dyn std::error::Error>> {
    tokio::signal::windows::ctrl_break()?.recv().await;
    Ok(())
}
