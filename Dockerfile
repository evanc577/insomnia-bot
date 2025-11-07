########
# chef #
########
FROM almalinux:10 AS chef

WORKDIR /insomnia_bot

# Install devel tools
RUN dnf group install -y "Development Tools"
RUN dnf install -y 'dnf-command(config-manager)'
RUN dnf config-manager --set-enabled crb
RUN dnf install -y python3-devel cmake opus-devel

# Install Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install cargo-chef
RUN cargo install cargo-chef


###########
# planner #
###########
FROM chef AS planner

COPY Cargo.toml .
COPY Cargo.lock .
COPY src/ ./src
RUN cargo chef prepare  --recipe-path recipe.json


###########
# builder #
###########
FROM chef AS builder

COPY --from=planner /insomnia_bot/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml .
COPY Cargo.lock .
COPY src/ ./src
RUN cargo build --release


##########
# Runner #
##########
FROM almalinux:10

RUN dnf install -y epel-release && \
    curl -L https://mirrors.rpmfusion.org/free/el/rpmfusion-free-release-$(rpm -E %rhel).noarch.rpm \
        -o /tmp/rpmfusion-free.rpm && \
    rpm -i /tmp/rpmfusion-free.rpm && rm /tmp/rpmfusion-free.rpm && \
    curl -L https://mirrors.rpmfusion.org/nonfree/el/rpmfusion-nonfree-release-$(rpm -E %rhel).noarch.rpm \
        -o /tmp/rpmfusion-nonfree.rpm && \
    rpm -i /tmp/rpmfusion-nonfree.rpm && rm /tmp/rpmfusion-nonfree.rpm && \
    dnf install -y 'dnf-command(config-manager)' && \
    dnf config-manager --enable crb && \
    dnf install -y ffmpeg python3 python3-pip && \
    dnf clean all && pip cache purge

WORKDIR /insomnia_bot
COPY --from=builder /insomnia_bot/target/release/insomnia-bot ./
ENTRYPOINT ["/insomnia_bot/insomnia-bot"]
