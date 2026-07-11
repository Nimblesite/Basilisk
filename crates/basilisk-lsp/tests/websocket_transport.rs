//! Integration tests for the WebSocket LSP transport ([LSPARCH-INVOKE]).
//!
//! These drive a REAL loopback WebSocket server, so they live in their own test
//! binary rather than the crate's unit-test pool: `cargo test --all-targets`
//! runs test binaries sequentially, so here only these few network tests are
//! active at once. In the 400+-test lib unit pool the shared CPU starves the
//! spawned server task and the timing-sensitive assertions flake. Each test also
//! waits for the listener to accept before connecting and retries the whole
//! scenario, so a transient scheduling race clears while a genuine defect (the
//! behaviour is deterministic) still fails every attempt.

use std::{error::Error, io, time::Duration};

use basilisk_lsp::websocket::run_server_ws;
use futures_util::{SinkExt as _, StreamExt as _};
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::http::{header::ORIGIN, HeaderValue};
use tokio_tungstenite::tungstenite::Message;

async fn spawn_test_server(
) -> Result<(String, tokio::task::JoinHandle<io::Result<()>>), Box<dyn Error>> {
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = reservation.local_addr()?.port();
    drop(reservation);

    let server = tokio::spawn(run_server_ws(port));

    // Poll until the listener is actually accepting rather than sleeping a fixed
    // interval: a fixed delay races the scheduler and the first real connect can
    // hit a not-yet-bound port. The probe is a raw TCP connect the server rejects
    // as a non-WebSocket handshake (and keeps listening), so it only proves
    // readiness.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_err()
    {
        if tokio::time::Instant::now() >= deadline {
            return Err("test WebSocket server never began accepting".into());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    Ok((format!("ws://127.0.0.1:{port}"), server))
}

/// Retry a contention-sensitive networking scenario until it reports the
/// expected outcome. Real infrastructure errors (`?`) fail immediately; a
/// scenario that reports `false` on every attempt also fails.
async fn assert_eventually<F, Fut>(label: &str, mut scenario: F) -> Result<(), Box<dyn Error>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<bool, Box<dyn Error>>>,
{
    for attempt in 1..=6_u32 {
        if scenario().await? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(u64::from(50 * attempt))).await;
    }
    Err(format!("`{label}` never held across retries").into())
}

// [LSPARCH-INVOKE] A web page must not be able to drive the localhost LSP with
// the user's filesystem authority. Native editor clients omit Origin.
#[tokio::test]
async fn rejects_browser_origin_and_keeps_native_clients() -> Result<(), Box<dyn Error>> {
    assert_eventually(
        "a browser Origin is rejected while native clients keep connecting",
        || async {
            let (url, server) = spawn_test_server().await?;
            let mut browser_request = url.clone().into_client_request()?;
            let _ = browser_request
                .headers_mut()
                .insert(ORIGIN, HeaderValue::from_static("https://attacker.example"));

            let browser_was_rejected = tokio_tungstenite::connect_async(browser_request)
                .await
                .is_err();
            let native_was_accepted = tokio_tungstenite::connect_async(&url).await.is_ok();
            server.abort();
            Ok(browser_was_rejected && native_was_accepted)
        },
    )
    .await
}

#[tokio::test]
async fn rejects_oversized_handshake_and_keeps_listening() -> Result<(), Box<dyn Error>> {
    assert_eventually(
        "an oversized handshake is rejected without terminating the listener",
        || async {
            let (url, server) = spawn_test_server().await?;
            let mut oversized_request = url.clone().into_client_request()?;
            let padding = vec![b'a'; 20 * 1024];
            let _ = oversized_request
                .headers_mut()
                .insert("x-padding", HeaderValue::from_bytes(&padding)?);

            let oversized_was_rejected = tokio_tungstenite::connect_async(oversized_request)
                .await
                .is_err();
            let native_was_accepted = tokio_tungstenite::connect_async(&url).await.is_ok();
            server.abort();
            Ok(oversized_was_rejected && native_was_accepted)
        },
    )
    .await
}

#[tokio::test]
async fn closes_connections_that_exceed_the_message_limit() -> Result<(), Box<dyn Error>> {
    assert_eventually("an oversized message closes the connection", || async {
        let (url, server) = spawn_test_server().await?;
        let Ok((mut client, _)) = tokio_tungstenite::connect_async(&url).await else {
            server.abort();
            return Ok(false); // transient connect race — retry
        };
        let text = format!(
            r#"{{"jsonrpc":"2.0","method":"$/oversized","params":"{}"}}"#,
            "a".repeat(8 * 1024 * 1024)
        );

        let send_result = client.send(Message::Text(text.into())).await;
        // The close is deterministic (tungstenite errors past max_message_size);
        // the generous deadline only absorbs scheduling delay under load.
        let was_closed = if send_result.is_err() {
            true
        } else {
            matches!(
                tokio::time::timeout(Duration::from_secs(15), client.next()).await,
                Ok(None | Some(Err(_) | Ok(Message::Close(_))))
            )
        };
        server.abort();
        Ok(was_closed)
    })
    .await
}
