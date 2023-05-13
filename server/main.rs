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
    conn: WebSocket<Upgraded>,
}

struct Game {
    food: Vec<(f32, f32)>,
    cells: Vec<Player>,
}

impl Default for Game {
    fn default() -> Self {
        let food = (0..100).map(|_| gen_food()).collect();
        Self {
            food,
            cells: Vec::new(),
        }
    }
}

async fn handle_client(
    fut: upgrade::UpgradeFut,
    game: Rc<RefCell<Game>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ws = fut.await?;

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

    game.borrow_mut().cells.push(Player {
        x: 0.0,
        y: 0.0,
        radius: 10.0,
        conn: ws,
    });

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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server started, listening on {}", "127.0.0.1:8080");
    let game = Rc::new(RefCell::new(Game::default()));
    let localset = tokio::task::LocalSet::new();
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
