# Cootsmania

[![Flow](https://github.com/kuviman/cootsmania/actions/workflows/flow.yml/badge.svg)](https://github.com/kuviman/cootsmania/actions/workflows/flow.yml)

![title](assets/ui/title.png)

[**PLAY THE GAME HERE**](https://kuviman.itch.io/cootsmania)

[CHANGELOG](CHANGELOG.md)

![plan](assets/ui/simple.png)

Use arrow keys to race around the house and get to Coots before other players do.

On each round, half of the players get eliminated.
Survive all the rounds and become the CHAMPION!

![screen](assets/ui/screenshot.png)

## GAMEPLAY

- Enter your name, pick your emote and a color for your car.  
- Wait for the current game to end.
- Each round, a text will be shown hinting where Coots is at the moment.
- If Coots is out of your screen, you will see an indicator showing where it is.
- Drift your way through the obstacles and reach coots.
- If you reach Coots earlier than half of the players, you are qualified for the next round.
- If you cannot reach Coots in 24 Seconds, you are eliminated.

## Made by

- [kuviman](https://github.com/kuviman) - Programming
- [Rincs](https://rincsart.com) - Art
- [Brainoid](https://twitter.com/brainoidgames) - Music & Sfx

## Build instructions

You will need a [Rust compiler](https://rustup.rs)

Then just running `cargo run --release` should compile (for a while) and start the game with local server so you can play against bots

## Running your own server

Easiest way to run your own server is to use provided [Dockerfile](Dockerfile).
It starts actual server and also serves the web client.

Example usage:

```sh
docker build -t cootsmania .
docker run --rm -it -p 8080:80 cootsmania
# Now open http://localhost:8080
```

Or just pull the already built image:

```sh
docker run --rm -it -p 8080:80 ghcr.io/kuviman/cootsmania
# Now open http://localhost:8080
```

Proxy via `nginx`/`caddy` to have https/wss.
