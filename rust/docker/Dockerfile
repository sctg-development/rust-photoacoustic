# Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
# This file is part of the rust-photoacoustic project and is licensed under the
# SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
#
# Dockerfile for building a performant static version of rust_photoacoustic
FROM alpine:3.23 AS libz-fpic-static-builder
ARG LIBZ_VERSION=1.2.13
RUN apk update && apk add \
    curl curl-dev curl-static clang git patch cmake build-base \
    openssl-dev alsa-utils alsaconf alsa-lib-dev \
    pkgconfig openssl-libs-static \
    musl-dev autoconf automake libtool \
    linux-headers
RUN cd /tmp && \
    curl -L https://zlib.net/fossils/zlib-${LIBZ_VERSION}.tar.gz | tar xz
RUN cd /tmp/zlib-${LIBZ_VERSION} && \
    CFLAGS="-fPIC -static" ./configure --static --prefix=/usr/local && \
    make -j$(nproc) && \
    make install

FROM alpine:3.23 AS openssl-static-builder
ARG OPENSSL_VERSION=3.6.0
RUN apk update && apk add clang git patch cmake build-base \
    pkgconfig musl-dev autoconf automake libtool \
    linux-headers
COPY --from=libz-fpic-static-builder /usr/local/lib/libz.a /usr/local/lib/libz.a
COPY --from=libz-fpic-static-builder /usr/local/include/zlib.h /usr/local/include/zlib.h
COPY --from=libz-fpic-static-builder /usr/local/lib/pkgconfig/zlib.pc /usr/local/lib/pkgconfig/zlib.pc
RUN cd /tmp && \
    wget https://github.com/openssl/openssl/releases/download/openssl-${OPENSSL_VERSION}/openssl-${OPENSSL_VERSION}.tar.gz --output-document - | tar xz
RUN cd /tmp/openssl-${OPENSSL_VERSION} && \
    ./Configure no-shared no-dso no-tests -fPIC \
    --prefix=/usr/local \
    --libdir=/usr/local/lib \
    --openssldir=/etc/ssl && \
    make -j$(nproc) && \
    make install_sw install_ssldirs

FROM alpine:3.23 AS librdkafka-static-builder
ARG LIBRDKAFKA_VERSION=2.12.1
RUN apk update && apk add \
    clang git patch cmake build-base \
    alsa-utils alsaconf alsa-lib-dev \
    pkgconfig \
    libpsl-static libpsl-dev\
    musl-dev autoconf automake libtool \
    linux-headers
# Copy static zlib built in the previous stage
COPY --from=libz-fpic-static-builder /usr/local/lib/libz.a /usr/local/lib/libz.a
COPY --from=libz-fpic-static-builder /usr/local/include/zlib.h /usr/local/include/zlib.h
COPY --from=libz-fpic-static-builder /usr/local/lib/pkgconfig/zlib.pc /usr/local/lib/pkgconfig/zlib.pc
COPY --from=openssl-static-builder /usr/local/lib/libssl.a /usr/local/lib64/libssl.a
COPY --from=openssl-static-builder /usr/local/lib/libcrypto.a /usr/local/lib64/libcrypto.a
COPY --from=openssl-static-builder /usr/local/include/openssl /usr/local/include/openssl
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/libcrypto.pc /usr/local/lib/pkgconfig/libcrypto.pc
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/libssl.pc /usr/local/lib/pkgconfig/libssl.pc
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/openssl.pc /usr/local/lib/pkgconfig/openssl.pc
RUN cd /tmp && \
    wget https://github.com/confluentinc/librdkafka/archive/refs/tags/v${LIBRDKAFKA_VERSION}.tar.gz --output-document - | tar xz
#Remove openssl dynamic libs to avoid issues with static linking
# RUN rm -f /usr/lib/libssl.so* && \
#     rm -f /usr/lib/libcrypto.so* && \
#     rm -f /usr/lib/libssl.so && \
#     rm -f /usr/lib/libcrypto.so
RUN cd /tmp/librdkafka-${LIBRDKAFKA_VERSION} && \
    ./configure --prefix=/usr/local \
    --enable-static \
    --disable-shared \
    --disable-gssapi \
    --disable-sasl \
    --disable-curl \
    --disable-lz4-ext \
    --cc=clang \
    --CFLAGS="-fPIC -static -O2" \
    --CPPFLAGS="-fPIC -static" \
    --CXXFLAGS="-fPIC -static -O2" \
    --LDFLAGS="-static -L /usr/local/lib -L /usr/lib" && \
    make -j$(nproc) || true && \
    make install && \
    # Remove any shared libraries that may have been created
    rm -f /usr/lib/librdkafka.so* /usr/local/lib/librdkafka.so*

FROM alpine:3.23 AS python-builder
ARG PYTHON_VERSION=3.12.12
ARG PYTHON_SHORT_VERSION=3.12

RUN apk update && apk add \
    clang git patch cmake build-base \
    curl curl-dev curl-static \
    alsa-utils alsaconf alsa-lib-dev \
    pkgconfig \
    musl-dev autoconf automake libtool \
    linux-headers

# Copy static zlib built in the previous stage
COPY --from=libz-fpic-static-builder /usr/local/lib/libz.a /usr/local/lib/libz.a
COPY --from=libz-fpic-static-builder /usr/local/include/zlib.h /usr/local/include/zlib.h
COPY --from=libz-fpic-static-builder /usr/local/lib/pkgconfig/zlib.pc /usr/local/lib/pkgconfig/zlib.pc
# Copy static openssl built in the previous stage
COPY --from=openssl-static-builder /usr/local/lib/libssl.a /usr/local/lib64/libssl.a
COPY --from=openssl-static-builder /usr/local/lib/libcrypto.a /usr/local/lib64/libcrypto.a
COPY --from=openssl-static-builder /usr/local/include/openssl /usr/local/include/openssl
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/libcrypto.pc /usr/local/lib/pkgconfig/libcrypto.pc
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/libssl.pc /usr/local/lib/pkgconfig/libssl.pc
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/openssl.pc /usr/local/lib/pkgconfig/openssl.pc
# Copy static librdkafka built in the previous stage
COPY --from=librdkafka-static-builder /usr/local/lib/librdkafka.a /usr/local/lib/librdkafka.a
COPY --from=librdkafka-static-builder /usr/local/include/librdkafka/ /usr/local/include/librdkafka/
COPY --from=librdkafka-static-builder /usr/local/lib/pkgconfig/rdkafka.pc /usr/local/lib/pkgconfig/rdkafka.pc
# Build mimalloc
RUN git clone https://github.com/microsoft/mimalloc.git &&\
    cd mimalloc &&\
    curl -sL https://github.com/tweag/rust-alpine-mimalloc/raw/b26002b49d466a295ea8b50828cb7520a71a872a/mimalloc.diff -o mimalloc.patch &&\
    patch --fuzz 50 -p1 < mimalloc.patch || true &&\
    mkdir build && cd build &&\
    cmake .. -DCMAKE_BUILD_TYPE=Release \
    -DMI_BUILD_SHARED=OFF \
    -DMI_BUILD_OBJECT=OFF \
    -DMI_BUILD_TESTS=OFF &&\
    make -j$(nproc) && make install

COPY --from=sctg/libtermcap:musl-1.3.2 /usr/local/lib/libtermcap.a /usr/local/lib/libtermcap.a
COPY --from=sctg/libtermcap:musl-1.3.2 /usr/local/include/termcap.h /usr/local/include/termcap.h

# Build libreadline statically from git tag readline-8.3
RUN cd /mimalloc/build && \
    curl -L https://ftp.gnu.org/gnu/readline/readline-8.3.tar.gz | tar xz && \
    cd readline-8.3 && \
    ./configure --prefix=/usr/local \
    --enable-static \
    --disable-shared \
    --enable-multibyte \
    --enable-year2038 \
    CFLAGS="-fPIC -static" && \
    make -j$(nproc) && \
    make install && \
    rm -fv /usr/lib/libreadline.so*

# Compile python and libpython3.12.2 statically
COPY rust/docker/static-modules.diff /static-modules.diff
RUN curl -sl https://www.python.org/ftp/python/$PYTHON_VERSION/Python-$PYTHON_VERSION.tgz | tar xz -C / && \
    cd /Python-$PYTHON_VERSION && \
    patch -p0 < /static-modules.diff && \
    ./configure --prefix=/usr/local \
    --disable-shared \
    --disable-test-modules \
    --with-openssl=/usr/local/ssl \
    --with-ensurepip=install \
    --enable-optimizations \
    --with-static-libpython \
    --disable-test-modules \
    ac_cv_func_preadv2=no \
    ac_cv_func_pwritev2=no \
    LDFLAGS="-static -L/usr/local/lib -L/usr/local/lib/mimalloc-2.2 -L/usr/local/lib64 -ltermcap -lreadline -lhistory -lmimalloc -lssl -lcrypto -lz" \
    CFLAGS="-static -fPIC" \
    CPPFLAGS="-static" \
    LINKFORSHARED=" " \
    DYNLOADFILE="dynload_stub.o" && \
    make -j$(nproc) python && \
    make -j$(nproc) install 2> /dev/null || true && \
    ln -s /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python && \
    ln -s /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python3 && \
    ln -s /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython3.a && \
    ln -s /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython.a && \
    ln -s /usr/local/include/python$PYTHON_SHORT_VERSION /usr/local/include/python3 && \
    mkdir -p /usr/local/lib/python${PYTHON_SHORT_VERSION} && \
    cp -r /Python-$PYTHON_VERSION/Lib/* /usr/local/lib/python${PYTHON_SHORT_VERSION}/ && \
    echo "Python and libpython$PYTHON_VERSION built and installed successfully"

RUN echo "Static Python build complete. Verifying installation..." && \
    python3 --version && \
    ldd $(which python3) || echo "ldd check skipped for static binary"

# Create PyO3 configuration file for static linking
RUN echo "implementation=CPython" > /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "version=$PYTHON_SHORT_VERSION" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "shared=false" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "abi3=false" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "lib_name=python$PYTHON_SHORT_VERSION" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "lib_dir=/usr/local/lib" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "executable=/usr/local/bin/python3" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "pointer_width=64" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "build_flags=" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "suppress_build_script_link_lines=false" >> /Python-$PYTHON_VERSION/pyo3-config.txt && \
    echo "include_dir=/usr/local/include/python$PYTHON_SHORT_VERSION" >> /Python-$PYTHON_VERSION/pyo3-config.txt

FROM alpine:3.23 AS builder
ARG PYTHON_VERSION=3.12.12
ARG PYTHON_SHORT_VERSION=3.12

COPY --from=python-builder /usr/local /usr/local
COPY --from=python-builder /mimalloc /mimalloc
COPY --from=python-builder /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python$PYTHON_SHORT_VERSION
COPY --from=python-builder /usr/local/lib/python$PYTHON_SHORT_VERSION /usr/local/lib/python$PYTHON_SHORT_VERSION
COPY --from=python-builder /Python-$PYTHON_VERSION/pyo3-config.txt /Python-$PYTHON_VERSION/pyo3-config.txt
COPY --from=libz-fpic-static-builder /usr/local/lib/libz.a /usr/local/lib/libz.a
COPY --from=libz-fpic-static-builder /usr/local/include/zlib.h /usr/local/include/zlib.h
COPY --from=libz-fpic-static-builder /usr/local/lib/pkgconfig/zlib.pc /usr/local/lib/pkgconfig/zlib.pc
COPY --from=openssl-static-builder /usr/local/lib/libssl.a /usr/local/lib64/libssl.a
COPY --from=openssl-static-builder /usr/local/lib/libcrypto.a /usr/local/lib64/libcrypto.a
COPY --from=openssl-static-builder /usr/local/include/openssl /usr/local/include/openssl
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/libcrypto.pc /usr/local/lib/pkgconfig/libcrypto.pc
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/libssl.pc /usr/local/lib/pkgconfig/libssl.pc
COPY --from=openssl-static-builder /usr/local/lib/pkgconfig/openssl.pc /usr/local/lib/pkgconfig/openssl.pc
COPY --from=librdkafka-static-builder /usr/local/lib/librdkafka.a /usr/local/lib/librdkafka.a
COPY --from=librdkafka-static-builder /usr/local/include/librdkafka/ /usr/local/include/librdkafka/
COPY --from=librdkafka-static-builder /usr/local/lib/pkgconfig/rdkafka.pc /usr/local/lib/pkgconfig/rdkafka.pc

RUN ln -sf /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python3 && \
    ln -sf /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python

RUN apk update && apk add \
    curl clang git patch cmake build-base \
    alsa-utils alsaconf alsa-lib-dev \
    pkgconfig \
    musl-dev autoconf automake libtool \
    linux-headers 

# Install node 22.x for building web assets for alpine  3.23
COPY --from=node:22-alpine3.22 /usr/local/bin/node /usr/local/bin/node
COPY --from=node:22-alpine3.22 /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s /usr/local/lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm && \
    ln -s /usr/local/lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx
RUN node -v && npm -v

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup.sh &&\
    chmod +x rustup.sh &&\
    ./rustup.sh -y 

# Add Rust to PATH for all subsequent commands
ENV PATH="/root/.cargo/bin:${PATH}"

# Replace memory allocation functions in Rust's libc with mimalloc for better performance
RUN cd /mimalloc/build && \
    LIBC_PATH=$(find ~/.rustup -name libc.a) && \
    echo "Found libc.a at: $LIBC_PATH" && \
    cp "$LIBC_PATH" libc_backup.a && \
    printf "CREATE libc.a\nADDLIB %s\nDELETE aligned_alloc.lo calloc.lo donate.lo free.lo libc_calloc.lo lite_malloc.lo malloc.lo malloc_usable_size.lo memalign.lo posix_memalign.lo realloc.lo reallocarray.lo valloc.lo\nADDLIB ./libmimalloc.a\nSAVE\n" "$LIBC_PATH" | ar -M && \
    mv libc.a "$LIBC_PATH" && \
    echo "Successfully patched libc.a with mimalloc"

# Set environment variables for PyO3 to use static Python
ENV PYO3_PYTHON=/usr/local/bin/python3
ENV PYO3_PYTHON_VERSION=$PYTHON_SHORT_VERSION
ENV PYTHON_SYS_EXECUTABLE=/usr/local/bin/python3
ENV PYO3_NO_PYTHON=0
ENV PYO3_CONFIG_FILE=/Python-$PYTHON_VERSION/pyo3-config.txt

# Additional environment variables for static linking
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

# Build the Rust application with additional environment variables
RUN cd rust && \
    cp config.example.yaml config.yaml

# Patch rdkafka-sys build.rs to add --disable-shared flag for static compilation
# This ensures librdkafka is compiled as a static library, not a shared library
# RUN if [ ! -f /usr/local/etc/rdkafka-sys-patched ]; then \
#     RDKAFKA_SYS_PATH="/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rdkafka-sys-4.9.0+2.10.0"; \
#     if [ -f "$RDKAFKA_SYS_PATH/build.rs" ]; then \
#     sed -i 's/println!("Compiling librdkafka");/configure_flags.push("--disable-shared".into()); println!("Compiling librdkafka");/' \
#     "$RDKAFKA_SYS_PATH/build.rs" && \
#     echo "Applied --disable-shared patch to rdkafka-sys build.rs" && \
#     touch /usr/local/etc/rdkafka-sys-patched; \
#     else \
#     echo "Warning: rdkafka-sys build.rs not found, will apply patch after first download"; \
#     fi; \
#     fi

# Set environment variables for all subsequent commands
ENV LIBRDKAFKA_SYS_USE_PKG_CONFIG=1
ENV RDKAFKA_SYS_USE_PKG_CONFIG=1  
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV RDKAFKA_SYS_STATIC=1
ENV RDKAFKA_SYS_STATIC_LIBRDKAFKA=1

# Copy the auto-initialize remover tool source
COPY rust/docker/remove-auto-init /rust-photoacoustic/rust/docker/remove-auto-init

# Build the auto-initialize remover tool (Rust)
RUN cd /rust-photoacoustic/rust/docker/remove-auto-init && \
    . /root/.cargo/env && \
    cargo build --release && \
    mkdir -p /usr/local/bin && \
    cp target/release/remove-auto-init /usr/local/bin/remove-auto-init

# Run the auto-initialize removal tool
RUN /usr/local/bin/remove-auto-init /rust-photoacoustic/rust/Cargo.toml

# Copy static libraries to /usr/local/lib for linking
COPY --from=python-builder /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a
COPY --from=python-builder /usr/local/lib/libtermcap.a /usr/local/lib/libtermcap.a
COPY --from=python-builder /usr/local/lib/libreadline.a /usr/local/lib/libreadline.a
COPY --from=python-builder /usr/local/lib/libhistory.a /usr/local/lib/libhistory.a
COPY --from=python-builder /usr/local/lib/mimalloc-2.2/libmimalloc.a /usr/local/lib/libmimalloc.a

# Build for different architectures with proper error handling
RUN cd /rust-photoacoustic/rust && \
    # Remove some .so for avoiding issues with static linking
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

# Set up runtime user for security
RUN adduser -D -s /bin/sh photoacoustic

# Create necessary directories
RUN mkdir -p /app/config /app/data && \
    chown -R photoacoustic:photoacoustic /app

# Copy default configuration
COPY --from=builder /rust-photoacoustic/rust/config.yaml /app/config/config.yaml
RUN chown photoacoustic:photoacoustic /app/config/config.yaml

# Create a script to copy binaries from the correct target directory
RUN mkdir -p /Python-$PYTHON_VERSION/copy-binaries

# Copy python and libpython3.12 statically built in the builder stage
COPY --from=python-builder /usr/local/bin/python$PYTHON_SHORT_VERSION /usr/local/bin/python$PYTHON_SHORT_VERSION
COPY --from=python-builder /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a
COPY --from=python-builder /usr/local/lib/python$PYTHON_SHORT_VERSION /usr/local/lib/python$PYTHON_SHORT_VERSION    

# Copy the target file created during build to determine the architecture
COPY --from=builder /rust-photoacoustic/rust/_target /Python-$PYTHON_VERSION/_target

# Copy binaries based on the target architecture
RUN TARGET=$(cat /Python-$PYTHON_VERSION/_target) && \
    echo "Copying binaries from target: $TARGET" && \
    mkdir -p /usr/local/bin

COPY --from=builder /rust-photoacoustic/rust/target/ /Python-$PYTHON_VERSION/target/

# Copy only existing binaries to avoid errors
RUN TARGET=$(cat /Python-$PYTHON_VERSION/_target) && \
    echo "Available binaries in /Python-$PYTHON_VERSION/target/$TARGET/release/:" && \
    ls -la "/Python-$PYTHON_VERSION/target/$TARGET/release/" && \
    for binary in photoacoustic create_token analyze_spectrum debug_config differential filters modbus_client noise_generator pid_tuner redis_viewer rs256keygen rust_photoacoustic; do \
    if [ -f "/Python-$PYTHON_VERSION/target/$TARGET/release/$binary" ]; then \
    echo "Copying $binary" && \
    cp "/Python-$PYTHON_VERSION/target/$TARGET/release/$binary" /usr/local/bin/ && \
    chmod +x "/usr/local/bin/$binary"; \
    else \
    echo "Warning: $binary not found in build output"; \
    fi; \
    done && \
    rm -rf /Python-$PYTHON_VERSION/target /Python-$PYTHON_VERSION/_target

USER photoacoustic
WORKDIR /app

# Expose the default port
EXPOSE 8080

# Default command
ENTRYPOINT ["/usr/local/bin/rust_photoacoustic"]
CMD ["--config", "/app/config/config.yaml"]