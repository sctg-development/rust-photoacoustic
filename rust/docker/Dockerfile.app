# Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
# This file is part of the rust-photoacoustic project and is licensed under the
# SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
#
# Dockerfile for building the final application
# Published as: sctg/rust-photoacoustic

FROM sctg/rust-photoacoustic-static-deps:latest AS deps
FROM sctg/rust-photoacoustic-python-builder:latest AS python-builder

FROM alpine:3.23 AS builder
ARG PYTHON_VERSION=3.12.12
ARG PYTHON_SHORT_VERSION=3.12

COPY --from=python-builder /usr/local /usr/local
COPY --from=python-builder /mimalloc /mimalloc
COPY --from=python-builder /Python-$PYTHON_VERSION/pyo3-config.txt /Python-$PYTHON_VERSION/pyo3-config.txt

# Copy static dependencies
COPY --from=deps /usr/local/lib/libz.a /usr/local/lib/libz.a
COPY --from=deps /usr/local/include/zlib.h /usr/local/include/zlib.h
COPY --from=deps /usr/local/lib/pkgconfig/zlib.pc /usr/local/lib/pkgconfig/zlib.pc
COPY --from=deps /usr/local/lib64/libssl.a /usr/local/lib64/libssl.a
COPY --from=deps /usr/local/lib64/libcrypto.a /usr/local/lib64/libcrypto.a
COPY --from=deps /usr/local/include/openssl /usr/local/include/openssl
COPY --from=deps /usr/local/lib/pkgconfig/libcrypto.pc /usr/local/lib/pkgconfig/libcrypto.pc
COPY --from=deps /usr/local/lib/pkgconfig/libssl.pc /usr/local/lib/pkgconfig/libssl.pc
COPY --from=deps /usr/local/lib/pkgconfig/openssl.pc /usr/local/lib/pkgconfig/openssl.pc
COPY --from=deps /usr/local/lib/librdkafka.a /usr/local/lib/librdkafka.a
COPY --from=deps /usr/local/include/librdkafka/ /usr/local/include/librdkafka/
COPY --from=deps /usr/local/lib/pkgconfig/rdkafka.pc /usr/local/lib/pkgconfig/rdkafka.pc

RUN ln -sf /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python3 && \
    ln -sf /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python

RUN apk update && apk add \
    curl clang git patch cmake build-base \
    alsa-utils alsaconf alsa-lib-dev \
    pkgconfig \
    musl-dev autoconf automake libtool \
    linux-headers 

# Install node 22.x for building web assets
COPY --from=node:22-alpine3.22 /usr/local/bin/node /usr/local/bin/node
COPY --from=node:22-alpine3.22 /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s /usr/local/lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm && \
    ln -s /usr/local/lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx
RUN node -v && npm -v

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup.sh &&\
    chmod +x rustup.sh &&\
    ./rustup.sh -y 

ENV PATH="/root/.cargo/bin:${PATH}"

# Replace memory allocation functions in Rust's libc with mimalloc
RUN cd /mimalloc/build && \
    LIBC_PATH=$(find ~/.rustup -name libc.a) && \
    echo "Found libc.a at: $LIBC_PATH" && \
    cp "$LIBC_PATH" libc_backup.a && \
    printf "CREATE libc.a\nADDLIB %s\nDELETE aligned_alloc.lo calloc.lo donate.lo free.lo libc_calloc.lo lite_malloc.lo malloc.lo malloc_usable_size.lo memalign.lo posix_memalign.lo realloc.lo reallocarray.lo valloc.lo\nADDLIB ./libmimalloc.a\nSAVE\n" "$LIBC_PATH" | ar -M && \
    mv libc.a "$LIBC_PATH" && \
    echo "Successfully patched libc.a with mimalloc"

# Set environment variables for PyO3
ENV PYO3_PYTHON=/usr/local/bin/python3
ENV PYO3_PYTHON_VERSION=$PYTHON_SHORT_VERSION
ENV PYTHON_SYS_EXECUTABLE=/usr/local/bin/python3
ENV PYO3_NO_PYTHON=0
ENV PYO3_CONFIG_FILE=/Python-$PYTHON_VERSION/pyo3-config.txt

ENV RUSTFLAGS="-C target-feature=+crt-static -L /usr/local/lib"
ENV PKG_CONFIG_PATH="/usr/local/lib/pkgconfig"
ENV PYTHON_CONFIGURE_OPTS="--enable-shared=no"
ENV CFLAGS="-I/usr/local/include/python$PYTHON_SHORT_VERSION"
ENV CPPFLAGS="-I/usr/local/include/python$PYTHON_SHORT_VERSION"

# Compile ALSA library statically
RUN cd /tmp && \
    wget https://www.alsa-project.org/files/pub/lib/alsa-lib-1.2.10.tar.bz2 && \
    tar -xjf alsa-lib-1.2.10.tar.bz2 && \
    cd alsa-lib-1.2.10 && \
    ./configure --prefix=/usr/local \
    --enable-static \
    --disable-shared \
    --disable-python \
    --disable-mixer \
    --disable-pcm-plugins \
    --disable-rawmidi \
    --disable-hwdep \
    --disable-seq \
    --disable-ucm \
    --disable-topology \
    --with-configdir=/usr/local/share/alsa \
    --with-plugindir=/usr/local/lib/alsa-lib \
    CC=clang \
    CFLAGS="-fPIC -static" && \
    make -j$(nproc) && \
    make install && \
    mv /usr/local/lib/libasound.a /usr/lib/libasound.a &&\
    rm -v /usr/lib/libasound.so*

RUN git clone --recurse-submodules https://github.com/sctg-development/rust-photoacoustic.git 
WORKDIR /rust-photoacoustic

RUN cd rust && \
    cp config.example.yaml config.yaml

ENV LIBRDKAFKA_SYS_USE_PKG_CONFIG=1
ENV RDKAFKA_SYS_USE_PKG_CONFIG=1  
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV RDKAFKA_SYS_STATIC=1
ENV RDKAFKA_SYS_STATIC_LIBRDKAFKA=1

COPY rust/docker/remove-auto-init /rust-photoacoustic/rust/docker/remove-auto-init

RUN cd /rust-photoacoustic/rust/docker/remove-auto-init && \
    . /root/.cargo/env && \
    cargo build --release && \
    mkdir -p /usr/local/bin && \
    cp target/release/remove-auto-init /usr/local/bin/remove-auto-init

RUN /usr/local/bin/remove-auto-init /rust-photoacoustic/rust/Cargo.toml

# Create staging directory with binaries for final image
RUN TARGET=$(cat /rust-photoacoustic/rust/_target) && \
    mkdir -p /rust-photoacoustic/rust/release-staging && \
    echo "Staging binaries from target: $TARGET" && \
    for binary in analyze_spectrum create_token debug_config differential filters modbus_client noise_generator pid_tuner redis_viewer rs256keygen rust_photoacoustic; do \
    if [ -f "/rust-photoacoustic/rust/target/$TARGET/release/$binary" ]; then \
    echo "Copying $binary to staging" && \
    cp "/rust-photoacoustic/rust/target/$TARGET/release/$binary" /rust-photoacoustic/rust/release-staging/ || true; \
    fi; \
    done && \
    echo "Staging directory contents:" && \
    ls -la /rust-photoacoustic/rust/release-staging/

# Clean cargo registry and git to save space
RUN rm -rf /root/.cargo/registry /root/.cargo/git && \
    rm -rf /rust-photoacoustic/rust/docker/remove-auto-init

# Copy static libraries to /usr/local/lib for linking
COPY --from=python-builder /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a
COPY --from=python-builder /usr/local/lib/libtermcap.a /usr/local/lib/libtermcap.a
COPY --from=python-builder /usr/local/lib/libreadline.a /usr/local/lib/libreadline.a
COPY --from=python-builder /usr/local/lib/libhistory.a /usr/local/lib/libhistory.a
COPY --from=python-builder /usr/local/lib/mimalloc-2.2/libmimalloc.a /usr/local/lib/libmimalloc.a

# Build for different architectures
RUN cd /rust-photoacoustic/rust && \
    rm -f /usr/local/lib/libpython3.so* && \
    rm -f /usr/local/lib/libpython$PYTHON_SHORT_VERSION.so* && \
    rm -f /usr/local/lib/libpython$PYTHON_SHORT_VERSION.so && \
    rm -f /usr/local/lib/libpython3.so && \
    rm -f /usr/lib/libcrypto.so* && \
    rm -f /usr/lib/libssl.so* && \
    rm -f /usr/lib/libcrypto.so && \
    if [ "$(uname -m)" = "armv7l" ]; then \
    echo "Building for armv7l architecture" && \
    . /root/.cargo/env && rustup target add armv7-unknown-linux-musleabihf && \
    ln -svf /usr/bin/ar /usr/bin/arm-linux-musleabihf-ar && \
    ln -svf /usr/bin/strip /usr/bin/arm-linux-musleabihf-strip && \
    ln -svf /usr/bin/ranlib /usr/bin/arm-linux-musleabihf-ranlib && \
    echo "armv7-unknown-linux-musleabihf" > _target && \
    LDFLAGS="-static -L/usr/local/lib -L/usr/local/lib64" RUSTFLAGS="-C target-feature=+crt-static -L /usr/local/lib -L /usr/local/lib64 -l static=readline -l static=history -l static=termcap -l static=mimalloc -l static=python$PYTHON_SHORT_VERSION -l static=rdkafka -l static=ssl -l static=crypto" CARGO_CFG_TARGET_FEATURE="+crt-static" cargo build --release --target armv7-unknown-linux-musleabihf && \
    echo "Build completed successfully for armv7l" && \
    ls -la target/armv7-unknown-linux-musleabihf/release/ || true; \
    elif [ "$(uname -m)" = "aarch64" ]; then \
    echo "Building for aarch64 architecture" && \
    . /root/.cargo/env && rustup target add aarch64-unknown-linux-musl && \
    ln -svf /usr/bin/ar /usr/bin/aarch64-linux-musl-ar && \
    ln -svf /usr/bin/strip /usr/bin/aarch64-linux-musl-strip && \
    ln -svf /usr/bin/ranlib /usr/bin/aarch64-linux-musl-ranlib && \
    echo "aarch64-unknown-linux-musl" > _target && \
    LDFLAGS="-static -L/usr/local/lib -L/usr/local/lib64" RUSTFLAGS="-C target-feature=+crt-static -L /usr/local/lib -L /usr/local/lib64 -l static=readline -l static=history -l static=termcap -l static=mimalloc -l static=python$PYTHON_SHORT_VERSION -l static=rdkafka -l static=ssl -l static=crypto" CARGO_CFG_TARGET_FEATURE="+crt-static" cargo build --release --target aarch64-unknown-linux-musl && \
    echo "Build completed successfully for aarch64" && \
    ls -la target/aarch64-unknown-linux-musl/release/ || true; \
    elif [ "$(uname -m)" = "x86_64" ]; then \
    echo "Building for x86_64 architecture" && \
    . /root/.cargo/env && rustup target add x86_64-unknown-linux-musl && \
    echo "x86_64-unknown-linux-musl" > _target && \
    LDFLAGS="-static -L/usr/local/lib -L/usr/local/lib64" RUSTFLAGS="-C target-feature=+crt-static -L /usr/local/lib -L /usr/local/lib64 -l static=readline -l static=history -l static=termcap -l static=mimalloc -l static=python$PYTHON_SHORT_VERSION -l static=rdkafka -l static=ssl -l static=crypto" CARGO_CFG_TARGET_FEATURE="+crt-static" cargo build --release --target x86_64-unknown-linux-musl && \
    echo "Build completed successfully for x86_64" && \
    ls -la target/x86_64-unknown-linux-musl/release/ || true; \
    else \
    echo "Unsupported architecture: $(uname -m)" && exit 1; \
    fi

FROM alpine:3.23 AS runtime
ARG PYTHON_VERSION=3.12.12
ARG PYTHON_SHORT_VERSION=3.12
ENV PYTHON_VERSION=$PYTHON_VERSION
ENV PYTHON_SHORT_VERSION=$PYTHON_SHORT_VERSION

RUN apk add --no-cache ca-certificates
RUN adduser -D -s /bin/sh photoacoustic
RUN mkdir -p /app/config /app/data && \
    chown -R photoacoustic:photoacoustic /app

COPY --from=builder /rust-photoacoustic/rust/config.yaml /app/config/config.yaml
RUN chown photoacoustic:photoacoustic /app/config/config.yaml

COPY --from=python-builder /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python$PYTHON_SHORT_VERSION
COPY --from=python-builder /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a
COPY --from=python-builder /usr/local/lib/python$PYTHON_SHORT_VERSION /usr/local/lib/python$PYTHON_SHORT_VERSION    

# Copy binaries from the staging directory
COPY --from=builder /rust-photoacoustic/rust/release-staging/* /usr/local/bin/

# Verify binaries are present
RUN for binary in analyze_spectrum create_token debug_config differential filters modbus_client noise_generator pid_tuner redis_viewer rs256keygen rust_photoacoustic; do \
    if [ ! -f "/usr/local/bin/$binary" ]; then \
    echo "Warning: $binary not found in staging"; \
    else \
    chmod +x "/usr/local/bin/$binary"; \
    fi; \
    done && \
    ls -la /usr/local/bin/

USER photoacoustic
WORKDIR /app
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/rust_photoacoustic"]
CMD ["--config", "/app/config/config.yaml"]
