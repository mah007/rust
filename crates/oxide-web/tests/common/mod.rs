//! Shared helpers for integration tests: a minimal raw HTTP/1.1 client used to
//! exercise the server directly (independent of the testing crate's client).

use std::net::SocketAddr;

use bytes::Bytes;
use http::{Request, header};
use http_body_util::{BodyExt, Empty};
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

/// Perform a single `GET` on a fresh connection.
pub async fn get_once(addr: SocketAddr, path: &str) -> Result<(u16, String), String> {
    let stream = TcpStream::connect(addr)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = http1::handshake(io)
        .await
        .map_err(|e| format!("handshake: {e}"))?;
    let task = tokio::spawn(async move {
        let _ = conn.await;
    });

    let req = Request::builder()
        .uri(path)
        .header(header::HOST, addr.to_string())
        .body(Empty::<Bytes>::new())
        .unwrap();
    let resp = sender
        .send_request(req)
        .await
        .map_err(|e| format!("send: {e}"))?;
    let status = resp.status().as_u16();
    let body = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("body: {e}"))?
        .to_bytes();

    task.abort();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

/// Send two sequential `GET`s over the *same* connection, returning both
/// statuses. Used to verify HTTP/1.1 keep-alive / connection reuse.
pub async fn two_on_one_connection(addr: SocketAddr, path: &str) -> Result<(u16, u16), String> {
    let stream = TcpStream::connect(addr)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = http1::handshake(io)
        .await
        .map_err(|e| format!("handshake: {e}"))?;
    let task = tokio::spawn(async move {
        let _ = conn.await;
    });

    let mut statuses = [0u16; 2];
    for slot in &mut statuses {
        let req = Request::builder()
            .uri(path)
            .header(header::HOST, addr.to_string())
            .body(Empty::<Bytes>::new())
            .unwrap();
        let resp = sender
            .send_request(req)
            .await
            .map_err(|e| format!("send: {e}"))?;
        *slot = resp.status().as_u16();
        // Fully drain the body so the connection is ready for the next request.
        resp.into_body()
            .collect()
            .await
            .map_err(|e| format!("body: {e}"))?;
    }

    task.abort();
    Ok((statuses[0], statuses[1]))
}
