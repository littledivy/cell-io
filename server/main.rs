use fastwebsockets::upgrade;
use fastwebsockets::OpCode;
use fastwebsockets::Frame;
use hyper::server::conn::Http;
use hyper::service::service_fn;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use tokio::net::TcpListener;
use common::Message;
use deku::prelude::*;

// Generate random food x and y coordinates.
fn gen_food() -> Message {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let x = rng.gen_range(-500.0..500.0);
    let y = rng.gen_range(-500.0..500.0);

    Message::SpawnFood(x, y)
}

fn msg_to_frame(msg: Message) -> Vec<u8> {
    msg.to_bytes().unwrap()
}

struct Player {
    x: f32,
    y: f32,
    radius: f32,
    conn: (),
}

struct Game {
   food: Vec<(f32, f32)>,
   cells: Vec<Player>,
}

async fn handle_client(
    fut: upgrade::UpgradeFut,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ws = fut.await?;

    for _ in 0..50 {
        let spawn_frame = Frame::new(true, OpCode::Binary, None, msg_to_frame(gen_food()).into());
        ws.write_frame(spawn_frame).await?;
    }
    
    loop {
        let frame = ws.read_frame().await?;
        match frame.opcode {
            OpCode::Close => break,
            _ => {}
        }
    }

    Ok(())
}

async fn server_upgrade(
    mut req: Request<Body>,
) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
    let (response, fut) = upgrade::upgrade(&mut req)?;

    tokio::task::spawn(async move {
        if let Err(e) = handle_client(fut).await {
            eprintln!("Error in websocket connection: {}", e);
        }
    });

    Ok(response)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server started, listening on {}", "127.0.0.1:8080");
    loop {
        let (stream, _) = listener.accept().await?;
        println!("Client connected");
        tokio::spawn(async move {
            let conn_fut = Http::new()
                .serve_connection(stream, service_fn(server_upgrade))
                .with_upgrades();
            if let Err(e) = conn_fut.await {
                println!("An error occurred: {:?}", e);
            }
        });
    }
}
