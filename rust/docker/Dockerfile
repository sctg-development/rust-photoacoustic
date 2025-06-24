# Dockerfile for building a performant static version of rust_photoacoustic
# Use Alpine stable instead of edge to avoid CMake compatibility issues
FROM alpine:3.18 AS builder

RUN apk update && apk add \
    curl clang git patch cmake build-base \
    openssl-dev alsa-utils alsaconf alsa-lib-dev \
    pkgconfig openssl-libs-static \
    zlib-dev zlib-static \
    musl-dev autoconf automake libtool \
    linux-headers \
    nodejs npm

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup.sh &&\
    chmod +x rustup.sh &&\
    ./rustup.sh -y &&\
    /root/.cargo/bin/rustup target add x86_64-unknown-linux-musl

# Add Rust to PATH for all subsequent commands
ENV PATH="/root/.cargo/bin:${PATH}"
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

# Replace memory allocation functions in Rust's libc with mimalloc for better performance
RUN cd /mimalloc/build && \
    LIBC_PATH=$(find ~/.rustup -name libc.a) && \
    echo "Found libc.a at: $LIBC_PATH" && \
    cp "$LIBC_PATH" libc_backup.a && \
    printf "CREATE libc.a\nADDLIB %s\nDELETE aligned_alloc.lo calloc.lo donate.lo free.lo libc_calloc.lo lite_malloc.lo malloc.lo malloc_usable_size.lo memalign.lo posix_memalign.lo realloc.lo reallocarray.lo valloc.lo\nADDLIB ./libmimalloc.a\nSAVE\n" "$LIBC_PATH" | ar -M && \
    mv libc.a "$LIBC_PATH" && \
    echo "Successfully patched libc.a with mimalloc"

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
    mv /usr/local/lib/libasound.a /usr/lib/libasound.a 

RUN git clone --recurse-submodules https://github.com/sctg-development/rust-photoacoustic.git 
WORKDIR /rust-photoacoustic

ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV CC_x86_64_unknown_linux_musl="clang"
ENV CXX_x86_64_unknown_linux_musl="clang++"

# Build the Rust application with additional environment variables
RUN cd rust && \
    cp config.example.yaml config.yaml && \
    LIBRDKAFKA_SYS_USE_PKG_CONFIG=1 \
    RDKAFKA_SYS_USE_PKG_CONFIG=1 \
    PKG_CONFIG_ALLOW_CROSS=1 
RUN cd rust &&\
    source ~/.cargo/env && cargo build --release --target x86_64-unknown-linux-musl --verbose

FROM alpine:3.18 AS runtime
COPY --from=builder /rust-photoacoustic/rust/target/release/photoacoustic /usr/local/bin/photoacoustic
COPY --from=builder /rust-photoacoustic/rust/target/release/create_token /usr/local/bin/create_token
COPY --from=builder /rust-photoacoustic/rust/target/release/analyze_spectrum /usr/local/bin/analyze_spectrum
COPY --from=builder /rust-photoacoustic/rust/target/release/debug_config /usr/local/bin/debug_config
COPY --from=builder /rust-photoacoustic/rust/target/release/differential /usr/local/bin/differential
COPY --from=builder /rust-photoacoustic/rust/target/release/filters /usr/local/bin/filters
COPY --from=builder /rust-photoacoustic/rust/target/release/modbus_client /usr/local/bin/modbus_client
COPY --from=builder /rust-photoacoustic/rust/target/release/noise_generator /usr/local/bin/noise_generator
COPY --from=builder /rust-photoacoustic/rust/target/release/pid_tuner /usr/local/bin/pid_tuner
COPY --from=builder /rust-photoacoustic/rust/target/release/redis_viewer /usr/local/bin/redis_viewer
COPY --from=builder /rust-photoacoustic/rust/target/release/rs256keygen /usr/local/bin/rs256keygen
COPY --from=builder /rust-photoacoustic/rust/target/release/rust_photoacoustic /usr/local/bin/rust_photoacoustic