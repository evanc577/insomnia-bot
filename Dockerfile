###########
# Builder #
###########
FROM almalinux:8 AS builder

# Install Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install Python
RUN dnf install -y python3-devel

# Install devel tools
RUN dnf group install -y "Development Tools"
RUN dnf install -y openssl-devel

# Compile
WORKDIR /insomnia_bot
COPY src/ ./src
COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo build --release


##########
# Runner #
##########
FROM almalinux:8

RUN dnf install -y 'dnf-command(config-manager)' epel-release \
    https://mirrors.rpmfusion.org/free/el/rpmfusion-free-release-8.noarch.rpm \
    https://mirrors.rpmfusion.org/nonfree/el/rpmfusion-nonfree-release-8.noarch.rpm
RUN dnf config-manager --set-enable powertools
RUN dnf install -y ffmpeg python3 python3-pip
RUN pip3 install yt-dlp ytmusicapi
RUN ln -s /usr/local/bin/yt-dlp /usr/local/bin/youtube-dl

WORKDIR /insomnia_bot
COPY --from=builder /insomnia_bot/target/release/insomnia-bot ./
ENTRYPOINT ["/insomnia_bot/insomnia-bot"]
