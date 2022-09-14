use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::debug;
use tokio::io::AsyncWriteExt;
use tokio::{
    net::TcpStream,
    sync::{
        mpsc::{UnboundedReceiver, UnboundedSender},
        RwLock,
    },
    task::JoinHandle,
};

use tokio::net::TcpSocket;

use super::{AllSinkTrait, CreateWriter};

pub struct ElasticSink {
    buffer: Arc<RwLock<Vec<Vec<u8>>>>,
    sender: UnboundedSender<Vec<u8>>,
    log_writer: JoinHandle<()>,
    pub log_flusher: JoinHandle<()>,
    log_stash_url: SocketAddr,
}

pub struct ElasticWriter {
    sender: UnboundedSender<Vec<u8>>,
}

impl ElasticSink {
    pub fn new(log_stash_url: SocketAddr) -> Self {
        let (sender, recv) = tokio::sync::mpsc::unbounded_channel();

        let buffer = Arc::new(RwLock::new(vec![]));
        let log_flusher = tokio::spawn(log_flusher_thread(log_stash_url, buffer.clone()));
        let log_writer = tokio::spawn(log_writer_thread(recv, buffer.clone()));
        let res = Self {
            buffer: buffer,
            sender: sender.clone(),
            log_flusher: log_flusher,
            log_writer: log_writer,
            log_stash_url,
        };

        res
    }
}

#[async_trait]
impl super::sink_trait::FinalizeLogs for ElasticSink {
    async fn finalize_logs(&self) {
        println!("Finalizing logs.");
        self.log_flusher.abort();
        let mut write_access = self.buffer.as_ref().write().await;

        if write_access.len() == 0 {
            self.log_writer.abort();
            println!("Finalized logs.");
            return;
        }

        let mut stream: TcpStream;
        loop {
            let opt = connect_to_socket(self.log_stash_url).await;

            match opt {
                Some(x) => {
                    stream = x;
                    break;
                }
                None => {
                    println!("Can't finalize logs!");
                    return;
                }
            }
        }

        while let Some(res) = write_access.pop() {
            let send_res = stream.write(&res).await;
            match send_res {
                Ok(size) => {
                    debug!("Send logs {:?}", size)
                }
                Err(err) => println!("Can't write logs to logstash server {:?}", err),
            }
        }

        self.log_writer.abort();
        println!("Finalized logs.");
    }
}

#[async_trait]
impl AllSinkTrait for ElasticSink {}

impl CreateWriter for ElasticSink {
    fn create_writer(&self) -> Box<(dyn std::io::Write + 'static)> {
        Box::new(ElasticWriter {
            sender: self.sender.clone(),
        })
    }
}

impl std::io::Write for ElasticWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::io::stdout().write_all(&buf).unwrap();

        match self.sender.send(buf.to_vec()) {
            Ok(r) => Ok(buf.len()),
            Err(err) => {
                println!("Can't write to elastic channel!");
                Err(std::io::Error::new(std::io::ErrorKind::Other, "Can't write to elastic channel!"))
            },
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Result::Ok(())
    }
}

async fn log_writer_thread(mut recv: UnboundedReceiver<Vec<u8>>, data: Arc<RwLock<Vec<Vec<u8>>>>) {
    while let Some(next_item) = recv.recv().await {
        let mut write_access = data.as_ref().write().await;
        write_access.push(next_item);
    }
}

//Executes each second
async fn log_flusher_thread<'a>(log_stash_url: SocketAddr, data: Arc<RwLock<Vec<Vec<u8>>>>) {
    let mut stream = get_lostash_tcp_stream(log_stash_url).await;

    loop {
        let mut write_access = data.as_ref().write().await;
        while let Some(res) = write_access.pop() {
            let send_res = stream.write(&res).await;
            match send_res {
                Ok(size) => {
                    debug!("Send logs {:?}", size)
                }
                Err(err) => {
                    println!("Can't write logs to logstash server {:?}", err);
                    write_access.push(res);
                    println!("Reconnect to logstash!");
                    stream = get_lostash_tcp_stream(log_stash_url).await;
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(250)).await
    }
}

async fn get_lostash_tcp_stream(log_stash_url: SocketAddr) -> TcpStream {
    loop {
        let opt = connect_to_socket(log_stash_url).await;

        match opt {
            Some(stream) => {
                return stream;
            }
            None => {
                println!("Can't connect to logstash! RETRY!");
                tokio::time::sleep(Duration::from_millis(1000)).await},
        }
    }
}

async fn connect_to_socket(log_stash_url: SocketAddr) -> Option<TcpStream> {
    let socket: TcpSocket;
    let socket_result = TcpSocket::new_v4();
    match socket_result {
        Ok(x) => socket = x,
        Err(err) => {
            println!("Can't create socket for logs {:?}", err);
            return None;
        }
    }
    let connect_res = socket.connect(log_stash_url).await;
    let stream: TcpStream;
    match connect_res {
        Ok(x) => {
            stream = x;
        }
        Err(err) => {
            println!("Can't connect to logstash server {:?}", err);
            return None;
        }
    }

    Some(stream)
}
