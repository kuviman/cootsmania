use super::*;

pub fn run(addr: &str) {
    for _ in 0..1000 {
        let addr = addr.to_owned();
        std::thread::spawn(move || {
            futures::executor::block_on(async {
                let mut connection: geng::net::client::Connection<ServerMessage, ClientMessage> =
                    geng::net::client::connect(&addr).await.unwrap();
                connection.send(ClientMessage::Ready(true));
                connection.send(ClientMessage::Ping);
                while let Some(message) = connection.next().await.transpose().unwrap() {
                    if let ServerMessage::Pong = message {
                        connection.send(ClientMessage::Ping);
                        connection.send(ClientMessage::UpdatePlayer(Player {
                            color: 0.0,
                            skin: 0,
                            pos: vec2::ZERO,
                            vel: vec2::ZERO,
                            rot: 0.0,
                        }));
                        std::thread::sleep_ms(50);
                    }
                }
            })
        });
    }
}
