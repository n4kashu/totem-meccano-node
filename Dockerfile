# FROM alpine:edge AS builder
# LABEL maintainer="chris.dcosta@totemaccounting.com"
# LABEL description="This is the build stage for Totem Meccano. Here we create the binary."

# RUN apk add build-base \
#     cmake \
#     linux-headers \
#     openssl-dev \
#     clang-dev \
#     cargo

# ARG PROFILE=release
# WORKDIR /totem-substrate

# COPY . /totem-substrate

# RUN cargo build --$PROFILE

# # ===== SECOND STAGE ======

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

# Note: We don't use Alpine and its packaged Rust/Cargo because they're too often out of date,
# preventing them from being used to build Substrate/Polkadot.

FROM phusion/baseimage:0.11 as builder
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the build stage for Totem Meccano. Here we create the binary."

ENV DEBIAN_FRONTEND=noninteractive

ARG PROFILE=release
WORKDIR /meccano

COPY . /meccano

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	export PATH="$PATH:$HOME/.cargo/bin" && \
	rustup toolchain install nightly && \
	rustup target add wasm32-unknown-unknown --toolchain nightly && \
	rustup default stable && \
	cargo build "--$PROFILE"

# ===== SECOND STAGE ======

FROM phusion/baseimage:0.11
LABEL maintainer="chris.dcosta@totemaccounting.com"
LABEL description="This is the 2nd stage: a very small image where we copy the Totem Meccano binary."
ARG PROFILE=release

RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	useradd -m -u 1000 -U -s /bin/sh -d /meccano meccano && \
	mkdir -p /meccano/.local/share/meccano && \
	chown -R meccano:meccano /meccano/.local && \
	ln -s /meccano/.local/share/meccano /data

COPY --from=builder /meccano/target/$PROFILE/totem-meccano /usr/local/bin
# COPY --from=builder /substrate/target/$PROFILE/subkey /usr/local/bin
# COPY --from=builder /substrate/target/$PROFILE/node-rpc-client /usr/local/bin
# COPY --from=builder /substrate/target/$PROFILE/node-template /usr/local/bin
# COPY --from=builder /substrate/target/$PROFILE/chain-spec-builder /usr/local/bin

# checks
RUN ldd /usr/local/bin/totem-meccano && \
	/usr/local/bin/totem-meccano --version

# Shrinking
RUN rm -rf /usr/lib/python* && \
	rm -rf /usr/bin /usr/sbin /usr/share/man

USER meccano
# EXPOSE 16181 9933 9944 9615
EXPOSE 16181 9933 9944
VOLUME ["/data"]

CMD ["/usr/local/bin/totem-meccano"]