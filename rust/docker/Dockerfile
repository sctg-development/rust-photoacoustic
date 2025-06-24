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
    ./rustup.sh -y 

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

# Build the Rust application with additional environment variables
RUN cd rust && \
    cp config.example.yaml config.yaml

# Set environment variables for all subsequent commands
ENV LIBRDKAFKA_SYS_USE_PKG_CONFIG=1
ENV RDKAFKA_SYS_USE_PKG_CONFIG=1  
ENV PKG_CONFIG_ALLOW_CROSS=1

# Build for different architectures with proper error handling
RUN cd rust && \
    if [ "$(uname -m)" = "armv7l" ]; then \
       echo "Building for armv7l architecture" && \
       . /root/.cargo/env && rustup target add armv7-unknown-linux-musleabihf && \
       ln -svf /usr/bin/ar /usr/bin/arm-linux-musleabihf-ar && \
       ln -svf /usr/bin/strip /usr/bin/arm-linux-musleabihf-strip && \
       ln -svf /usr/bin/ranlib /usr/bin/arm-linux-musleabihf-ranlib && \
       echo "armv7-unknown-linux-musleabihf" > _target && \
       cargo build --release --target armv7-unknown-linux-musleabihf --verbose && \
       echo "Build completed successfully for armv7l" && \
       ls -la target/armv7-unknown-linux-musleabihf/release/; \
    elif [ "$(uname -m)" = "aarch64" ]; then \
       echo "Building for aarch64 architecture" && \
       . /root/.cargo/env && rustup target add aarch64-unknown-linux-musl && \
       ln -svf /usr/bin/ar /usr/bin/aarch64-linux-musl-ar && \
       ln -svf /usr/bin/strip /usr/bin/aarch64-linux-musl-strip && \
       ln -svf /usr/bin/ranlib /usr/bin/aarch64-linux-musl-ranlib && \
       echo "aarch64-unknown-linux-musl" > _target && \
       cargo build --release --target aarch64-unknown-linux-musl --verbose && \
       echo "Build completed successfully for aarch64" && \
       ls -la target/aarch64-unknown-linux-musl/release/; \
    elif [ "$(uname -m)" = "x86_64" ]; then \
       echo "Building for x86_64 architecture" && \
       . /root/.cargo/env && rustup target add x86_64-unknown-linux-musl && \
       echo "x86_64-unknown-linux-musl" > _target && \
       cargo build --release --target x86_64-unknown-linux-musl --verbose && \
       echo "Build completed successfully for x86_64" && \
       ls -la target/x86_64-unknown-linux-musl/release/; \
    else \
       echo "Unsupported architecture: $(uname -m)" && exit 1; \
    fi

FROM alpine:3.18 AS runtime
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
RUN mkdir -p /tmp/copy-binaries

# Copy the target file created during build to determine the architecture
COPY --from=builder /rust-photoacoustic/rust/_target /tmp/_target

# Copy binaries based on the target architecture
RUN TARGET=$(cat /tmp/_target) && \
    echo "Copying binaries from target: $TARGET" && \
    mkdir -p /usr/local/bin

COPY --from=builder /rust-photoacoustic/rust/target/ /tmp/target/

# Copy only existing binaries to avoid errors
RUN TARGET=$(cat /tmp/_target) && \
    echo "Available binaries in /tmp/target/$TARGET/release/:" && \
    ls -la "/tmp/target/$TARGET/release/" && \
    for binary in photoacoustic create_token analyze_spectrum debug_config differential filters modbus_client noise_generator pid_tuner redis_viewer rs256keygen rust_photoacoustic; do \
        if [ -f "/tmp/target/$TARGET/release/$binary" ]; then \
            echo "Copying $binary" && \
            cp "/tmp/target/$TARGET/release/$binary" /usr/local/bin/ && \
            chmod +x "/usr/local/bin/$binary"; \
        else \
            echo "Warning: $binary not found in build output"; \
        fi; \
    done && \
    rm -rf /tmp/target /tmp/_target

USER photoacoustic
WORKDIR /app

# Expose the default port
EXPOSE 8080

# Default command
ENTRYPOINT ["/usr/local/bin/rust_photoacoustic"]
CMD ["--config", "/app/config/config.yaml"]