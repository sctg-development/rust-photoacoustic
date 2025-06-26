# Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
# This file is part of the rust-photoacoustic project and is licensed under the
# SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
#
# Dockerfile for building a performant static version of rust_photoacoustic
# Use Alpine stable instead of edge to avoid CMake compatibility issues
FROM alpine:3.18 AS builder
ARG PYTHON_VERSION=3.12.11
ARG PYTHON_SHORT_VERSION=3.12

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

# Compile python and libpython3.12.2 statically
RUN curl -sl https://www.python.org/ftp/python/$PYTHON_VERSION/Python-$PYTHON_VERSION.tgz | tar xz -C / && \
    cd /Python-$PYTHON_VERSION && \
    ./configure --prefix=/usr/local \
                --disable-shared \
                --disable-test-modules \
                --with-openssl=/usr/local/ssl \
                --with-ensurepip=install \
                --enable-optimizations \
                --with-static-libpython \
                LDFLAGS="-static -L/usr/local/lib" \
                CFLAGS="-static -fPIC" \
                CPPFLAGS="-static" \
                LINKFORSHARED=" " \
                DYNLOADFILE="dynload_stub.o" && \
    make -j$(nproc) python && \
    cp /Python-$PYTHON_VERSION/python /usr/local/bin/python3 && \
    cp /Python-$PYTHON_VERSION/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a && \
    # Copy Python headers - essential for PyO3 compilation
    mkdir -p /usr/local/include/python$PYTHON_SHORT_VERSION && \
    cp -r /Python-$PYTHON_VERSION/Include/* /usr/local/include/python$PYTHON_SHORT_VERSION/ && \
    cp /Python-$PYTHON_VERSION/pyconfig.h /usr/local/include/python$PYTHON_SHORT_VERSION/ && \
    ln -s /usr/local/bin/python3 /usr/local/bin/python && \
    ln -s /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython3.a && \
    ln -s /usr/local/lib/libpython$PYTHON_SHORT_VERSION.a /usr/local/lib/libpython.a && \
    ln -s /usr/local/include/python$PYTHON_SHORT_VERSION /usr/local/include/python3 && \
    echo "Python and libpython$PYTHON_VERSION built and installed successfully"

# Set environment variables for PyO3 to use static Python
ENV PYO3_PYTHON=/usr/local/bin/python3
ENV PYO3_PYTHON_VERSION=$PYTHON_SHORT_VERSION
ENV PYTHON_SYS_EXECUTABLE=/usr/local/bin/python3
ENV PYO3_NO_PYTHON=0
ENV PYO3_CONFIG_FILE=/Python-$PYTHON_VERSION/pyo3-config.txt

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

# Additional environment variables for static linking
ENV RUSTFLAGS="-C target-feature=+crt-static -L /usr/local/lib"
ENV PKG_CONFIG_PATH="/usr/local/lib/pkgconfig"
ENV PYTHON_CONFIGURE_OPTS="--enable-shared=no"
ENV CFLAGS="-I/usr/local/include/python$PYTHON_SHORT_VERSION"
ENV CPPFLAGS="-I/usr/local/include/python$PYTHON_SHORT_VERSION"

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
       cargo build --release --target armv7-unknown-linux-musleabihf --feature static --verbose && \
       echo "Build completed successfully for armv7l" && \
       ls -la target/armv7-unknown-linux-musleabihf/release/; \
    elif [ "$(uname -m)" = "aarch64" ]; then \
       echo "Building for aarch64 architecture" && \
       . /root/.cargo/env && rustup target add aarch64-unknown-linux-musl && \
       ln -svf /usr/bin/ar /usr/bin/aarch64-linux-musl-ar && \
       ln -svf /usr/bin/strip /usr/bin/aarch64-linux-musl-strip && \
       ln -svf /usr/bin/ranlib /usr/bin/aarch64-linux-musl-ranlib && \
       echo "aarch64-unknown-linux-musl" > _target && \
       cargo build --release --target aarch64-unknown-linux-musl --feature static --verbose && \
       echo "Build completed successfully for aarch64" && \
       ls -la target/aarch64-unknown-linux-musl/release/; \
    elif [ "$(uname -m)" = "x86_64" ]; then \
       echo "Building for x86_64 architecture" && \
       . /root/.cargo/env && rustup target add x86_64-unknown-linux-musl && \
       echo "x86_64-unknown-linux-musl" > _target && \
       cargo build --release --target x86_64-unknown-linux-musl --feature static --verbose && \
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
RUN mkdir -p /Python-$PYTHON_VERSION/copy-binaries

# Copy python and libpython3.13.5 statically built in the builder stage
COPY --from=builder /usr/local/bin/python3 /usr/local/bin/python3
COPY --from=builder /usr/local/lib/libpython3.13.a /usr/local/lib/libpython3.13.a

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