pub mod domain;
pub mod generated_proto;
pub mod services;

#[cfg(test)]
pub mod test {
    use rust_service_sdk::{
        app::{
            app_ctx::{GetGlobalState, GetLogStashUrl, InitGrpc},
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

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SettingsModel {
        #[serde(rename = "RustServiceTemplateTest")]
        pub inner: SettingsModelInner,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SettingsModelInner {

        #[serde(rename = "LogStashUrl")]
        pub log_stash_url: String,

        #[serde(rename = "MyNoSqlWriterUrl")]
        pub my_no_sql_writer_url: String,

        #[serde(rename = "MyNoSqlReaderHostPort")]
        pub my_mo_sql_reader_host_port: String,
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
        pub async fn new(_settings: SettingsModel) -> Self {
            Self {
                states: GlobalStates::new(),
                database: Arc::new(DatabaseImpl::new()),
                some_counter: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl InitGrpc for AppContext {
        fn init_grpc(
            &self,
            server: Box<std::cell::RefCell<tonic::transport::Server>>,
        ) -> tonic::transport::server::Router {
            let bookstore = BookStoreImpl::new(self.database.clone());

            server.borrow_mut().add_service(
                crate::generated_proto::bookstore_server::BookstoreServer::new(bookstore),
            )
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

    #[derive(Serialize, Deserialize, Debug)]
    struct TestEntity {
        #[serde(rename = "PartitionKey")]
        pub partition_key: String,
        #[serde(rename = "RowKey")]
        pub row_key: String,
        #[serde(rename = "TimeStamp")]
        pub time_stamp: String,
        #[serde(rename = "data")]
        pub data: i64,
    }

    impl my_no_sql_server_abstractions::MyNoSqlEntity for TestEntity {
        fn get_partition_key(&self) -> &str {
            &self.partition_key[..]
        }
        fn get_row_key(&self) -> &str {
            &self.row_key[..]
        }
        fn get_time_stamp(&self) -> i64 {
            0
        }
    }
    
    //Uses env settings
    //Integration test
    #[tokio::test]
    async fn check_that_sdk_works() {
        let mut application = 
        Application::<AppContext, SettingsModel>::init(AppContext::new).await;

        let clone = application.context.clone();
        let func = move |server| clone.init_grpc(server);

        let data_writer = my_no_sql_data_writer::MyNoSqlDataWriter::<TestEntity>::new(
            application.settings.inner.my_no_sql_writer_url.clone(),
            "rust-test-entity".to_string(),
            true,
            true,
            my_no_sql_server_abstractions::DataSyncronizationPeriod::Sec15,
        );

        let _ = data_writer.create_table_if_not_exists().await.unwrap();
        let entity = TestEntity {
            data: 1,
            partition_key: "Test".into(),
            row_key: "Row".into(),
            time_stamp: "2022-30-08T14:26:04.8276".into(),
        };
        data_writer.insert_or_replace_entity(&entity).await.unwrap();
        let res = data_writer
            .get_entity(&entity.partition_key, &entity.row_key)
            .await
            .unwrap();
        println!("{:?}", res.unwrap());
/*         let no_sql_connection = my_no_sql_tcp_reader::MyNoSqlTcpConnection::new(
            application
                .settings
                .inner
                .my_mo_sql_reader_host_port
                .clone(),
            "sdk-test".to_string(),
        );

        let logger: Arc<dyn rust_extensions::Logger + Send + Sync> = Arc::new(TestLogger {});

        no_sql_connection.start(logger).await;
        let data_reader = no_sql_connection
            .get_reader::<TestEntity>("rust-test-entity".to_string())
            .await;

        loop {
            let entity = data_reader.get_entity("Test", "Row").await;

            if let Some(x) = entity {
                println!("{:?}", x);
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(1_000)).await;
        } */

        let sink = application.start_hosting(func,
        "rust_service_template".into()).await;

        //JUST A GRPC EXAMPLE
        let token = Arc::new(CancellationToken::new());
        let client_pereodic_task =
            tokio::spawn(start_test(application.env_config.clone(), token.clone()));
        let mut running_tasks = vec![client_pereodic_task];

        application
            .wait_for_termination(
                sink,
                &mut running_tasks,
                Some(token.clone()),
                graceful_shutdown_func,
                555,
            )
            .await;

        //Check grpc
        println!("Assert");
        let counter = application.context.clone().database.read().await;
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
