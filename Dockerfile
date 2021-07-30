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
	command -v wasm-gc || \
	cargo +nightly-2019-10-14 install --git https://github.com/alexcrichton/wasm-gc --force && \
    ./scripts/build.sh && \
	cargo "$buildtype" "--$PROFILE"

# ===== SECOND STAGE ======

FROM phusion/baseimage:0.11
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the 2nd stage: a very small image where we copy the Totem binary."

RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	useradd -m -u 1000 -U -s /bin/sh -d /totem totemadmin && \
	mkdir -p /totem/.local/share/totem-meccano && \
	chown -R totemadmin:totemadmin /totem/.local && \
	ln -s /totem/.local/share/totem-meccano /data

COPY --from=builder /totem/target/release/totem-meccano /usr/local/bin/

# checks
RUN ldd /usr/local/bin/totem-meccano && \
	/usr/local/bin/totem-meccano --version

# Shrinking
RUN rm -rf /usr/lib/python* && \
	rm -rf /usr/bin /usr/sbin /usr/share/man

USER totemadmin

EXPOSE 16181 9933 9944

VOLUME ["/data"]

CMD ["/usr/local/bin/totem-meccano"]



# ===== OLD ======







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