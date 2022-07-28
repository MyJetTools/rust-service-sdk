pub mod domain;
pub mod generated_proto;
pub mod services;

#[cfg(test)]
pub mod test {

    use rust_service_sdk::{
        app::{
            app_ctx::{GetGlobalState, GetLogStashUrl},
            global_states::GlobalStates,
        },
        application::Application,
        configuration::EnvConfig,
    };
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio_util::sync::CancellationToken;

    use crate::{
        domain::{Database, DatabaseImpl, RequestCounter},
        services::BookStoreImpl,
    };

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SettingsModel {
        #[serde(rename = "RustServiceTemplateTest")]
        pub inner: SettingsModelInner,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SettingsModelInner {
        #[serde(rename = "ZipkinUrl")]
        pub zipkin_url: String,

        #[serde(rename = "SeqServiceUrl")]
        pub seq_service_url: String,

        #[serde(rename = "LogStashUrl")]
        pub log_stash_url: String,
    }

    impl GetLogStashUrl for SettingsModel {
        fn get_logstash_url(&self) -> String {
            self.inner.log_stash_url.clone()
        }
    }

    pub struct AppContext {
        pub states: GlobalStates,
        pub database: Arc<dyn Database<RequestCounter> + Sync + Send>,
        pub some_counter: Arc<Mutex<u64>>,
    }

    impl AppContext {
        pub fn new(settings: &SettingsModel) -> Self {
            Self {
                states: GlobalStates::new(),
                database: Arc::new(DatabaseImpl::new()),
                some_counter: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl GetGlobalState for AppContext {
        fn is_initialized(&self) -> bool {
            self.states.is_initialized()
        }

        fn is_shutting_down(&self) -> bool {
            self.states.is_shutting_down()
        }

        fn shutting_down(&self) {
            self.states
                .shutting_down
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    //Uses env settings
    //Integration test
    #[tokio::test]
    async fn check_that_sdk_works() {
        let application = Application::<AppContext, SettingsModel>::init(AppContext::new).await;

        let context = application.context.clone();
        let sink = application.start_logger();
        let (grpc_server, http_server) = application
            .start_hosting(move |server| {
                let bookstore = BookStoreImpl::new(context.database.clone());

                server.borrow_mut().add_service(
                    crate::generated_proto::bookstore_server::BookstoreServer::new(bookstore),
                )
            })
            .await;

        //JUST A GRPC EXAMPLE
        let token = Arc::new(CancellationToken::new());
        let client_pereodic_task =
            tokio::spawn(start_test(application.env_config.clone(), token.clone()));
        let mut running_tasks = vec![client_pereodic_task];

        application
            .wait_for_termination(
                sink,
                grpc_server,
                http_server,
                &mut running_tasks,
                Some(token.clone()),
                graceful_shutdown_func,
                555,
            )
            .await;

        //Check grpc
        println!("Assert");
        let counter = application.context.database.read().await;
        assert!(counter.counter == 1);

        //check shutdown
        let counter = application.context.some_counter.lock().await;
        assert!(*counter == 1);

        println!("Assert Finished");
    }

    async fn graceful_shutdown_func(context: Arc<AppContext>) -> bool {
        let mut guard = context.some_counter.lock().await;
        *guard += 1;
        true
    }

    async fn start_test(
        endpoint: Arc<EnvConfig>,
        token: Arc<CancellationToken>,
    ) -> Result<(), anyhow::Error> {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let mut client = crate::generated_proto::bookstore_client::BookstoreClient::connect(
            format!("http://{}:{}", endpoint.base_url, endpoint.grpc_port),
        )
        .await?;

        let request =
            tonic::Request::new(crate::generated_proto::GetBookRequest { id: "123".into() });

        let response = client.get_book(request).await.unwrap();

        println!("RESPONSE={:?}", response);
        token.cancel();
        Ok(())
    }
}
