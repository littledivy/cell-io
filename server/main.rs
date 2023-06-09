use common::Message;
use deku::prelude::*;
use fastwebsockets::upgrade;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::WebSocket;
use hyper::server::conn::Http;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use tokio::net::TcpListener;
use tokio::sync::broadcast::{self, channel};

#[derive(Clone)]
struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + 'static,
    Fut::Output: 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn_local(fut);
    }
}

// Generate random food x and y coordinates.
fn gen_food() -> (f32, f32) {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let x = rng.gen_range(-500.0..500.0);
    let y = rng.gen_range(-500.0..500.0);
    (x, y)
}

fn msg_to_frame(msg: Message) -> Vec<u8> {
    msg.to_bytes().unwrap()
}

struct Player {
    x: f32,
    y: f32,
    radius: f32,
    uid: u32,
}

struct Game {
    food: Vec<(f32, f32)>,
    cells: Vec<Player>,
    incoming: broadcast::Sender<Message>,
    broadcast: broadcast::Sender<Message>,
}

impl Game {
    fn new(incoming: broadcast::Sender<Message>, broadcast: broadcast::Sender<Message>) -> Self {
        let food = (0..100).map(|_| gen_food()).collect();
        Self {
            food,
            cells: Vec::new(),
            incoming,
            broadcast,
        }
    }
}

async fn handle_client(
    fut: upgrade::UpgradeFut,
    game: Rc<RefCell<Game>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ws = fut.await?;

    let tx = game.borrow().incoming.clone();
    let mut outgoing = game.borrow().broadcast.subscribe();
    
    // Spawn food.
    let frames = game
        .borrow()
        .food
        .iter()
        .map(|(x, y)| {
            let msg = Message::SpawnFood(*x, *y);
            Frame::new(true, OpCode::Binary, None, msg_to_frame(msg).into())
        })
        .collect::<Vec<_>>();

    for frame in frames {
        ws.write_frame(frame).await?;
    }

    // Spawn cells.
    let frames = game
        .borrow()
        .cells
        .iter()
        .map(|cell| {
            let msg = Message::NewPlayer(cell.x, cell.y, cell.uid);
            Frame::new(true, OpCode::Binary, None, msg_to_frame(msg).into())
        })
        .collect::<Vec<_>>();

    for frame in frames {
        ws.write_frame(frame).await?;
    }

    let uid = {
      let mut game = game.borrow_mut();
      let uid = game.cells.len() as u32;
      game.cells.push(Player {
          x: 0.0,
          y: 0.0,
          radius: 10.0,
          uid,
      });
      uid
    };

    // Spawn the player.
    let msg = Message::Start(0.0, 0.0, uid);
    tx.send(Message::NewPlayer(0.0, 0.0, uid)).unwrap();
    let frame = Frame::new(true, OpCode::Binary, None, msg_to_frame(msg).into());
    ws.write_frame(frame).await?;
    println!("Spawned player with uid {}", uid);
    
    loop {
        tokio::select! {
            frame = ws.read_frame() => {
              let frame = frame?;
              match frame.opcode {
                OpCode::Binary => {
                let msg = common::Message::try_from(frame.payload.as_ref())?;
                tx.send(msg)?;
            },
            _ => {},
              }            }
            msg = outgoing.recv() => {
                let msg = msg?;
                if msg.uid() == Some(uid) {
                    continue;
                }
                let frame = Frame::new(true, OpCode::Binary, None, msg_to_frame(msg).into());
                ws.write_frame(frame).await?;
            }
        }
    }

    Ok(())
}

async fn server_upgrade(
    mut req: Request<Body>,
    game: Rc<RefCell<Game>>,
) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
    let (response, fut) = upgrade::upgrade(&mut req)?;

    tokio::task::spawn_local(async move {
        if let Err(e) = handle_client(fut, game).await {
            eprintln!("Error in websocket connection: {}", e);
        }
    });

    Ok(response)
}

async fn game_loop(
    game: Rc<RefCell<Game>>,
    mut incoming_rx: broadcast::Receiver<Message>,
    mut outgoing_tx: broadcast::Sender<Message>,
) {
    // Players send its events, here we actually handle them.
    loop {
        let msg = incoming_rx.recv().await.unwrap();
        match msg {
            Message::NewPlayer(x, y, uid) => {
                // Broadcast new player to all players.
                let msg = Message::NewPlayer(x, y, uid);
                outgoing_tx.send(msg).unwrap();
            }
            Message::MovePlayer(x, y, uid) => {
                // Broadcast move player to all players.
                let msg = Message::MovePlayer(x, y, uid);
                outgoing_tx.send(msg).unwrap();
            }
            _ => {}
        }
    }
}

const BROADCAST_BUFFER_SIZE: usize = 128;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server started, listening on {}", "127.0.0.1:8080");

    // Initialize the game state.
    let (incoming_tx, incoming_rx) = channel(BROADCAST_BUFFER_SIZE);
    let (outgoing_tx, _) = channel(BROADCAST_BUFFER_SIZE);
    let game = Rc::new(RefCell::new(Game::new(incoming_tx, outgoing_tx.clone())));

    let localset = tokio::task::LocalSet::new();

    localset.spawn_local(game_loop(game.clone(), incoming_rx, outgoing_tx));

    // Spawn a task that will listen for incoming connections.
    localset
        .run_until(async move {
            loop {
                let (stream, _) = listener.accept().await?;
                println!("Client connected");
                let game = game.clone();
                tokio::task::spawn_local(async move {
                    let conn_fut = Http::new()
                        .with_executor(SpawnExecutor)
                        .serve_connection(
                            stream,
                            service_fn(move |req| {
                                let game = game.clone();
                                async move { server_upgrade(req, game).await }
                            }),
                        )
                        .with_upgrades();
                    if let Err(e) = conn_fut.await {
                        println!("An error occurred: {:?}", e);
                    }
                });
            }
        })
        .await
}
