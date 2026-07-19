//! Server-lifecycle integration tests: graceful shutdown and connection reuse.

mod common;

use std::time::Duration;

use oxide_web::{Application, ServerConfig, routing::get, serve_on};
use oxide_web_testing::TestServer;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

use common::{get_once, two_on_one_connection};

async fn ok() -> &'static str {
    "ok"
}

#[tokio::test]
async fn graceful_shutdown_drains_then_stops_accepting() {
    let service = Application::new()
        .route("/", get(ok))
        .into_service()
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let mut config = ServerConfig::default();
    config.graceful_shutdown_timeout = Duration::from_secs(5);

    let (tx, rx) = oneshot::channel::<()>();
    let server = tokio::spawn(serve_on(vec![listener], service, config, async move {
        let _ = rx.await;
    }));

    // The server responds normally before shutdown.
    let (status, body) = get_once(addr, "/").await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(body, "ok");

    // Trigger graceful shutdown and wait for the server task to finish draining.
    tx.send(()).unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server did not shut down within the timeout")
        .expect("server task panicked");
    assert!(outcome.is_ok(), "serve_on returned an error: {outcome:?}");

    // New connections are refused once the listener is dropped. Retry briefly to
    // avoid racing the OS socket teardown.
    let mut refused = false;
    for _ in 0..40 {
        if TcpStream::connect(addr).await.is_err() {
            refused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        refused,
        "server should stop accepting connections after shutdown"
    );
}

#[tokio::test]
async fn connection_is_reused_for_multiple_requests() {
    let server = TestServer::start(Application::new().route("/", get(ok)))
        .await
        .unwrap();

    let (first, second) = two_on_one_connection(server.addr(), "/").await.unwrap();
    assert_eq!(first, 200);
    assert_eq!(second, 200);
}
