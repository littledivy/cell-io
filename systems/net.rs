use common::Message;
use crossbeam_channel::Sender;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use deku::prelude::*;
use deku::DekuContainerRead;
use fastwebsockets::{Frame, OpCode, WebSocket};
use hyper::header::CONNECTION;
use hyper::header::UPGRADE;
use hyper::upgrade::Upgraded;
use hyper::Body;
use hyper::Request;
use std::future::Future;
use tokio::net::TcpStream;

const SERVER_ADDR: &str = "localhost:8080";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

async fn ws_connect() -> Result<WebSocket<Upgraded>> {
    let stream = TcpStream::connect(SERVER_ADDR).await?;

    let req = Request::builder()
        .method("GET")
        .uri(format!("http://{}/", SERVER_ADDR))
        .header("Host", SERVER_ADDR)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(
            "Sec-WebSocket-Key",
            fastwebsockets::handshake::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .body(Body::empty())?;

    let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, stream).await?;
    Ok(ws)
}

pub async fn connect(tx: Sender<Message>, mut player_rx: UnboundedReceiver<Message>) -> Result<()> {
    let mut ws = ws_connect().await?;

    // Send spawn message.
    let msg = Message::NewPlayer(0.0, 0.0);
    let frame = Frame::new(true, OpCode::Binary, None, msg.to_bytes().unwrap().into());
    ws.write_frame(frame).await?;

    loop {
        tokio::select! {
            frame = ws.read_frame() => {
                let frame = frame?;
                match frame.opcode {
                  OpCode::Binary => {
                    let msg = Message::try_from(frame.payload.as_ref())?;
                    tx.send(msg)?;
                  },
                  _ => {},
                }            
            }
            Some(msg) = player_rx.recv() => {
                let frame = Frame::new(true, OpCode::Binary, None, msg.to_bytes().unwrap().into());
                ws.write_frame(frame).await?;
            }
        }
    }
}
