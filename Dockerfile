# Usage

# docker build \ 
# --build-arg buildtype=check .

# docker build \
# --build-arg buildtype=build .

FROM phusion/baseimage:0.11 as builder
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the build stage for Totem Node Types. Here we create the binary."

ENV DEBIAN_FRONTEND=noninteractive

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
	# the following probably deprecated due to updates to rust compiler
	# command -v wasm-gc || \
	# cargo +nightly-2019-10-14 install --git https://github.com/alexcrichton/wasm-gc --force && \
    ./scripts/build.sh && \
	cargo "$buildtype" "--$PROFILE"

# ===== SECOND STAGE ======

FROM phusion/baseimage:0.11
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the 2nd stage: a very small image where we copy the Totem binary."

RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	useradd -m -u 1000 -U -s /bin/sh -d /totem-substrate totemadmin && \
	mkdir -p /totem-substrate/.local/share/totem-meccano && \
	chown -R totemadmin:totemadmin /totem-substrate/.local && \
	ln -s /totem-substrate/.local/share/totem-meccano /data

COPY --from=builder /totem-substrate/target/release/totem-meccano /usr/local/bin/

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