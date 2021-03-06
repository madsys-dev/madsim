//! a tokio Tcp Simulatior
//!
//! # Examples
//!
//! ```
//!
//! use madsim::{net::{NetSim, TcpStream, TcpListener}, plugin, runtime::Runtime, time::timeout};
//! use std::{net::SocketAddr, sync::Arc, time::Duration};
//! use tokio::{
//!     io::{AsyncReadExt, AsyncWriteExt},
//!     sync::Barrier,
//! };
//!
//!
//! let runtime = Runtime::new();
//! let addr1 = "10.0.0.1:1".parse::<SocketAddr>().unwrap();
//! let addr2 = "10.0.0.2:1".parse::<SocketAddr>().unwrap();
//! let node1 = runtime.create_node().ip(addr1.ip()).build();
//! let node2 = runtime.create_node().ip(addr2.ip()).build();
//! let barrier = Arc::new(Barrier::new(2));
//!
//! let barrier_ = barrier.clone();
//! let f1 = node1.spawn(async move {
//!     let listener = TcpListener::bind(addr1).await.unwrap();
//!     barrier_.wait().await;
//!     let (mut stream, _) = listener.accept().await.unwrap();
//!     stream.write(b"hello world").await.unwrap();
//!     stream.flush().await.unwrap();
//!     stream
//! });
//!
//! let f2 = node2.spawn(async move {
//!     barrier.wait().await;
//!     let mut stream = TcpStream::connect(addr1).await.unwrap();
//!     let mut buf = [0; 20];
//!     let len = stream.read(&mut buf).await.unwrap();
//!     assert_eq!(&buf[0..len], b"hello world");
//! });
//!
//! runtime.block_on(f1).unwrap();
//! runtime.block_on(f2).unwrap();
//! ```

use std::any::Any;

/// tcp packet payload
pub type Payload = Box<dyn Any + Send + Sync>;

mod config;
mod listener;
mod stream;

pub use self::config::*;
pub use self::listener::*;
pub use self::stream::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{net::NetSim, plugin, runtime::Runtime, time::timeout};
    use std::{net::SocketAddr, sync::Arc, time::Duration};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        sync::Barrier,
    };

    #[test]
    fn send_recv() {
        let runtime = Runtime::new();
        let addr1 = "10.0.0.1:1".parse::<SocketAddr>().unwrap();
        let addr2 = "10.0.0.2:1".parse::<SocketAddr>().unwrap();
        let node1 = runtime.create_node().ip(addr1.ip()).build();
        let node2 = runtime.create_node().ip(addr2.ip()).build();
        let barrier = Arc::new(Barrier::new(2));
        let barrier_ = barrier.clone();

        let f1 = node1.spawn(async move {
            let listener = TcpListener::bind(addr1).await.unwrap();
            barrier_.wait().await;
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write(b"hello world").await.unwrap();
            stream.flush().await.unwrap();
            stream
        });

        let f2 = node2.spawn(async move {
            barrier.wait().await;
            let mut stream = TcpStream::connect(addr1).await.unwrap();
            let mut buf = [0; 20];
            let len = stream.read(&mut buf).await.unwrap();
            assert_eq!(&buf[0..len], b"hello world");
        });

        runtime.block_on(f1).unwrap();
        runtime.block_on(f2).unwrap();
    }

    #[test]
    fn disconnect_and_recovery() {
        let runtime = Runtime::new();
        let addr1 = "10.0.0.1:1".parse::<SocketAddr>().unwrap();
        let addr2 = "10.0.0.2:1".parse::<SocketAddr>().unwrap();
        let node1 = runtime.create_node().ip(addr1.ip()).build();
        let node2 = runtime.create_node().ip(addr2.ip()).build();
        let id1 = node1.id();
        let id2 = node2.id();
        let barrier = Arc::new(Barrier::new(2));
        let barrier_ = barrier.clone();

        let f1 = node1.spawn(async move {
            // phase1
            let net = plugin::simulator::<NetSim>();
            net.disconnect(id1);
            let listener = TcpListener::bind(addr1).await.unwrap();
            barrier.wait().await;

            // phase2
            timeout(Duration::from_secs(1), listener.accept())
                .await
                .expect_err("listener should not get connection");
            barrier.wait().await;

            // phase3
            net.connect(id1);
            barrier.wait().await;
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write(b"hello world").await.unwrap();
            stream.flush().await.unwrap();
            barrier.wait().await;

            // phase4
            net.disconnect2(id1, id2);
            crate::task::spawn(async move {
                crate::time::sleep(Duration::from_secs(5)).await;
                net.connect2(id1, id2);
            });
            barrier.wait().await;
            stream.write(b"hello world").await.unwrap();
            stream.flush().await.unwrap();

            stream
        });

        let f2 = node2.spawn(async move {
            // phase1
            barrier_.wait().await;

            // phase2
            TcpStream::connect(addr1)
                .await
                .expect_err("connect should fail");
            barrier_.wait().await;

            // phase3
            barrier_.wait().await;
            let mut stream = TcpStream::connect(addr1).await.unwrap();
            let mut buf = [0; 20];
            let len = stream.read(&mut buf).await.unwrap();
            assert_eq!(&buf[0..len], b"hello world");
            barrier_.wait().await;

            // phase4
            barrier_.wait().await;
            let len = stream.read(&mut buf).await.unwrap();
            assert_eq!(&buf[0..len], b"hello world");

            stream
        });

        runtime.block_on(f1).unwrap();
        runtime.block_on(f2).unwrap();
    }

    #[test]
    fn reset() {
        let runtime = Runtime::new();
        let addr1 = "10.0.0.1:1".parse::<SocketAddr>().unwrap();
        let addr2 = "10.0.0.2:1".parse::<SocketAddr>().unwrap();
        let node1 = runtime.create_node().ip(addr1.ip()).build();
        let node2 = runtime.create_node().ip(addr2.ip()).build();
        let barrier = Arc::new(Barrier::new(3));
        let barrier_ = barrier.clone();
        let barrier__ = barrier.clone();

        let f1 = node1.spawn(async move {
            let listener = TcpListener::bind(addr1).await.unwrap();
            barrier.wait().await;
            let (mut stream, _) = listener.accept().await.unwrap();
            barrier.wait().await;
            barrier.wait().await;

            stream.write(b"hello world").await.unwrap();
            stream.flush().await.expect_err("write should fail");
        });

        let f2 = node2.spawn(async move {
            barrier_.wait().await;
            let mut stream = TcpStream::connect(addr1).await.unwrap();
            barrier_.wait().await;
            barrier_.wait().await;

            let mut buf = [0; 20];
            let len = stream.read(&mut buf).await.expect("read should return EOF");
            assert_eq!(len, 0);
        });

        runtime.block_on(async move {
            barrier__.wait().await;
            barrier__.wait().await;
            let net = plugin::simulator::<NetSim>();
            net.reset_node(node1.id());
            barrier__.wait().await;

            f1.await.unwrap();
            f2.await.unwrap();
        });
    }
}
