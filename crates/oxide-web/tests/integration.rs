//! End-to-end HTTP integration tests driven through the `oxide-web-testing`
//! harness (real server on an ephemeral port).

use std::sync::Arc;

use oxide_web::{Application, Params, RemoteAddr, Request, StatusCode, routing::get};
use oxide_web_testing::TestServer;

async fn index() -> &'static str {
    "Hello, world!"
}

async fn health() -> &'static str {
    "OK"
}

async fn show_user(req: Request) -> String {
    let params = req.extensions().get::<Params>().expect("params present");
    format!("user {}", params.get("id").unwrap_or("?"))
}

async fn whoami(req: Request) -> String {
    match req.extensions().get::<RemoteAddr>() {
        Some(addr) => format!("peer {}", addr.addr().ip()),
        None => "no peer".to_owned(),
    }
}

fn app() -> Application {
    Application::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/users/:id", get(show_user))
        .route("/whoami", get(whoami))
}

#[tokio::test]
async fn get_root_returns_200_plain_text() {
    let server = TestServer::start(app()).await.unwrap();
    server
        .get("/")
        .await
        .assert_ok()
        .assert_text("Hello, world!")
        .assert_header("content-type", "text/plain; charset=utf-8");
}

#[tokio::test]
async fn unknown_path_returns_404() {
    let server = TestServer::start(app()).await.unwrap();
    server.get("/nope").await.assert_not_found();
}

#[tokio::test]
async fn wrong_method_returns_405_with_allow_header() {
    let server = TestServer::start(app()).await.unwrap();
    server
        .post("/", "")
        .await
        .assert_status(StatusCode::METHOD_NOT_ALLOWED)
        .assert_header("allow", "GET");
}

#[tokio::test]
async fn path_parameter_is_captured() {
    let server = TestServer::start(app()).await.unwrap();
    server
        .get("/users/42")
        .await
        .assert_ok()
        .assert_text("user 42");
}

#[tokio::test]
async fn remote_addr_is_available_to_handlers() {
    let server = TestServer::start(app()).await.unwrap();
    server
        .get("/whoami")
        .await
        .assert_ok()
        .assert_body_contains("peer 127.0.0.1");
}

#[tokio::test]
async fn shared_state_is_accessible() {
    #[derive(Clone)]
    struct AppState {
        service_name: &'static str,
    }

    async fn name(req: Request) -> String {
        req.extensions()
            .get::<AppState>()
            .map(|s| s.service_name.to_owned())
            .unwrap_or_default()
    }

    let app = Application::new()
        .route("/name", get(name))
        .with_state(AppState {
            service_name: "oxide",
        });

    let server = TestServer::start(app).await.unwrap();
    server.get("/name").await.assert_ok().assert_text("oxide");
}

#[tokio::test]
async fn handles_many_concurrent_requests() {
    let server = Arc::new(TestServer::start(app()).await.unwrap());

    let mut handles = Vec::new();
    for _ in 0..64 {
        let server = Arc::clone(&server);
        handles.push(tokio::spawn(
            async move { server.get("/health").await.status() },
        ));
    }

    for handle in handles {
        assert_eq!(handle.await.unwrap(), StatusCode::OK);
    }
}

#[tokio::test]
async fn custom_fallback_handler_runs() {
    async fn missing() -> (StatusCode, &'static str) {
        (StatusCode::NOT_FOUND, "nothing here")
    }

    let app = Application::new().route("/", get(index)).fallback(missing);

    let server = TestServer::start(app).await.unwrap();
    server
        .get("/does-not-exist")
        .await
        .assert_not_found()
        .assert_text("nothing here");
}
