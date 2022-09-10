###########
# Builder #
###########
FROM almalinux:9 AS builder

# Install Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install devel tools
RUN dnf group install -y "Development Tools"
RUN dnf config-manager --set-enabled crb
RUN dnf install -y python3-devel cmake opus-devel

# Compile
WORKDIR /insomnia_bot
COPY src/ ./src
COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo build --release


##########
# Runner #
##########
FROM almalinux:9-minimal

RUN microdnf install -y epel-release
RUN curl -L https://mirrors.rpmfusion.org/free/el/rpmfusion-free-release-$(rpm -E %rhel).noarch.rpm \
    -o /tmp/rpmfusion-free.rpm && \
    rpm -i /tmp/rpmfusion-free.rpm && rm /tmp/rpmfusion-free.rpm
RUN curl -L https://mirrors.rpmfusion.org/nonfree/el/rpmfusion-nonfree-release-$(rpm -E %rhel).noarch.rpm \
    -o /tmp/rpmfusion-nonfree.rpm && \
    rpm -i /tmp/rpmfusion-nonfree.rpm && rm /tmp/rpmfusion-nonfree.rpm
RUN microdnf install -y ffmpeg python3 python3-pip
RUN pip3 install yt-dlp ytmusicapi

WORKDIR /insomnia_bot
COPY --from=builder /insomnia_bot/target/release/insomnia-bot ./
ENTRYPOINT ["/insomnia_bot/insomnia-bot"]
