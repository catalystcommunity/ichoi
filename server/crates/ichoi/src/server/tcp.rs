//! The CSIL/TCP surface (§4). Pre-alpha: line-delimited JSON envelopes (the same seam as the
//! WebSocket path) over plain TCP. Production is CSIL-Events over TLS with three pinned,
//! auto-rotating server keys (§4.2) — TLS + `csilgen-transport` framing are TODO (§16).

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use crate::handlers::{App, Ctx, Identity};
use crate::transport;

pub async fn serve_tcp(app: App, addr: String) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    log::info!("CSIL/TCP (pre-alpha line-JSON) listening on {addr}");
    loop {
        let (stream, _peer) = listener.accept().await?;
        let app = app.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, app).await {
                log::debug!("tcp connection closed: {e}");
            }
        });
    }
}

async fn handle_conn(stream: tokio::net::TcpStream, app: App) -> anyhow::Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();
    let ctx = Ctx {
        identity: Identity::Anonymous,
    };
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let app2 = app.clone();
        let ctx2 = ctx.clone();
        let reply =
            tokio::task::spawn_blocking(move || transport::handle_json(&app2, &ctx2, &line))
                .await?;
        write_half.write_all(reply.as_bytes()).await?;
        write_half.write_all(b"\n").await?;
    }
    Ok(())
}
