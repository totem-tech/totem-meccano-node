# FROM alpine:edge AS builder
# LABEL maintainer="chris.dcosta@totemaccounting.com"
# LABEL description="This is the build stage for Totem Meccano. Here we create the binary."

FROM phusion/baseimage:0.11 as builder
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the build stage for Totem Node Types. Here we create the binary."

ENV DEBIAN_FRONTEND=noninteractive

# RUN apk add build-base \
#     cmake \
#     linux-headers \
#     openssl-dev \
#     clang-dev \
# 	curl \
#     cargo

ARG PROFILE=release

## Use check or build
ARG buildtype

WORKDIR /totem-substrate

COPY . /totem-substrate

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	export PATH="$PATH:$HOME/.cargo/bin" && \
	rustup toolchain install nightly-2019-10-14 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2019-10-14 && \
	rustup default nightly-2019-10-14 && \
    cargo "$buildtype" "--$PROFILE"

# ===== SECOND STAGE ======




# RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
# 	export PATH="$PATH:$HOME/.cargo/bin" && \
# 	rustup toolchain install nightly-2019-10-14 && \
# 	rustup default nightly-2019-10-14 && \
# 	rustup target add wasm32-unknown-unknown --toolchain nightly-2019-10-14 && \
# 	rustup default nightly-2019-10-14 && \
# 	cargo check --$PROFILE

# RUN cargo build --$PROFILE

# ===== SECOND STAGE ======

# FROM alpine:edge
# LABEL maintainer="chris.dcosta@totemaccounting.com"
# LABEL description="This is the 2nd stage: a very small image where we copy the Totem Meccano binary."
# ARG PROFILE=release
# COPY --from=builder /totem-substrate/target/$PROFILE/totem-meccano /usr/local/bin

# RUN apk add --no-cache ca-certificates \
#     libstdc++ \
#     openssl

# RUN rm -rf /usr/lib/python* && \
# 	mkdir -p /root/.local/share/Meccano && \
# 	ln -s /root/.local/share/Meccano /data

# EXPOSE 16181 9933 9944
# VOLUME ["/data"]

# ENTRYPOINT ["/usr/local/bin/totem-meccano"]