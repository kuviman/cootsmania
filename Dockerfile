FROM kuviman/geng AS builder

RUN apt update && apt install --yes libudev-dev
WORKDIR /src
# First create a layer with built dependencies to cache them in separate docker layer
COPY Cargo.toml .
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    cargo build --release --target wasm32-unknown-unknown && \
    rm -rf src
# Now actually compile the project
ARG CONNECT
COPY . .
RUN touch src/main.rs && \
    cargo geng build --release --web && \
    mv target/geng target/web && \
    cargo geng build --release && \
    mv target/geng target/server && \
    echo DONE

# Now create a small image
FROM debian:bullseye-slim
WORKDIR /root
RUN apt update && apt install --yes \
    libasound2-dev \
    libfreetype-dev \
    wget
RUN mkdir caddy && cd caddy && \
    wget https://github.com/caddyserver/caddy/releases/download/v2.6.4/caddy_2.6.4_linux_amd64.tar.gz && \
    tar -xzf caddy_2.6.4_linux_amd64.tar.gz
COPY --from=builder /src/target/web web
COPY --from=builder /src/target/server server
COPY Caddyfile start.sh .
CMD ["/bin/bash", "start.sh"]
EXPOSE 80