FROM alpine:edge AS builder
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the build stage for Totem Meccano. Here we create the binary."

RUN apk add build-base \
    cmake \
    linux-headers \
    openssl-dev \
    clang-dev \
    cargo

ARG PROFILE=release
WORKDIR /totem-substrate

COPY . /totem-substrate

RUN cargo build --$PROFILE

# ===== SECOND STAGE ======

FROM alpine:edge
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the 2nd stage: a very small image where we copy the Totem Meccano binary."
ARG PROFILE=release
COPY --from=builder /totem-substrate/target/$PROFILE/totem-meccano /usr/local/bin

RUN apk add --no-cache ca-certificates \
    libstdc++ \
    openssl

RUN rm -rf /usr/lib/python* && \
	mkdir -p /root/.local/share/Meccano && \
	ln -s /root/.local/share/Meccano /data

EXPOSE 16181 9933 9944
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/totem-meccano"]