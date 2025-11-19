![](https://tokeisrv.sctg.eu.org/b1/github.com/sctg-development/rust-photoacoustic?type=Rust,TypeScript,TSX,C&category=code)
![](https://tokeisrv.sctg.eu.org/b1/github.com/sctg-development/rust-photoacoustic?type=TSX,Rust,TypeScript&category=comments)  
![](https://tokeisrv.sctg.eu.org/b1/github.com/sctg-development/rust-photoacoustic?type=Markdown)
# Flexible Gas Analyzer by Laser Photoacoustic Spectroscopy

## Development Philosophy & Disclaimer

This project follows a true continuous integration approach where development happens transparently in the main branch. This allows anyone to observe the real-time evolution of a complex scientific application from early stages through completion. Tests are designed and implemented in real-time with the code. This ensures that the continuous integration process not only verifies code functionality but also guarantees that development consistently aligns with the project's objectives.

**Current Status:** This project is actively under development and not yet ready for production use. You're seeing the unfiltered development process - including experiments, refactoring, and iterative improvements.

**What This Means:**

- The codebase may be incomplete or contain non-working components
- APIs and architecture may change significantly between commits
- Tests are an integral part of development and may occasionally fail as new features are integrated or refactored, reflecting the ongoing alignment with project goals.
- Documentation is being written alongside the code and evolves with it

Rather than hiding work-in-progress behind feature branches, this approach demonstrates how modern software development progresses through incremental improvements, refactoring, and testing. Feel free to watch, star, or fork this repository to follow its evolution from prototype to working application.

**License Note:** This code is provided as-is, without warranty, under the terms specified in the LICENSE file.

## Configuration

The application can be configured using a YAML configuration file. By default, it looks for `config.yaml` in the current directory.

### Configuration File

You can specify an alternative configuration file using the `--config` command line argument:

```bash
cargo run --bin rust_photoacoustic -- --server --config /path/to/your/config.yaml
```

If the specified configuration file doesn't exist, a default one will be generated.

### Example Configuration

```yaml
# Visualization server settings
visualization:
  port: 8080                    # The port to listen on
  address: 127.0.0.1            # The address to bind to
  name: LaserSmartApiServer/0.1.0 # The server name
  # SSL certificate PEM data (Base64 encoded) - Optional
  # cert: LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0tLS0t...
  # SSL key PEM data (Base64 encoded) - Optional
  # key: LS0tLS1CRUdJTiBQUklWQVRFIEtFWS0tLS0t...
  # HMAC secret for JWT token signing - Optional, default value will be used if not provided
  hmac_secret: my-super-secret-jwt-key-for-photoacoustic-app
```

### SSL Certificates

The application automatically generates self-signed SSL certificates during the build process if they don't already exist. These certificates are stored in the `resources/` directory and are included in the binary at compile time.

You don't need to manually create certificates for development purposes, but for production use, you can replace them with proper certificates. You can:

1. Generate custom certificates using the utility function in `src/utility/certificate_utilities.rs`
2. Directly add your own certificates to the `resources/` directory
3. Specify base64-encoded certificates in the configuration file

The build script will not overwrite existing certificate files.

For production use you can put your own certificates in the configuration file like this:

```yaml
visualization:
  cert: |
    LS0tLS1CRUdJTiBDRVJUSUZJ0FURV0tLS0tLS0tCk1JSURnRENDQW9TZ0F3SUJBZ0lSQU5aR2d3Z1N3bG9wY2d4cG9iTjV6bXh4c2xvY3p6c2xvY3p6c2xvY3p6Ck1B
  key: | 
    LS0tLS1CRUdJTiBQUklWQVRFIEtFWS
```

This allows you to use your own certificates without needing to modify the code or build scripts.

### Command Line Overrides

Command line arguments take precedence over configuration file values:

- `--web-port` overrides `visualization.port`
- `--web-address` overrides `visualization.address`
- `--hmac-secret` overrides `visualization.hmac_secret`

### JWT Authentication

The application uses JSON Web Tokens (JWT) for authentication. The HMAC secret used for signing and verifying JWTs can be configured in the following ways:

1. In the configuration file:

```yaml
visualization:
  # Other settings...
  hmac_secret: your-secure-jwt-secret-key
```

2. Via command line:

```bash
cargo run -- --hmac-secret your-secure-jwt-secret-key
```

For production deployments, it is strongly recommended to set a custom HMAC secret rather than using the default.

## Project Objective

Develop a Rust program to analyze the concentration of water vapor in air using laser photoacoustic spectroscopy in a differential Helmholtz resonator. The goal is to process the sound signal to extract the amplitude of the fundamental component related to photoacoustic excitation.

## Physical Principle

- **Photoacoustic Spectroscopy**: A laser pulsed modulated to the cell's resonance frequency (typically 2 kHz). The laser beam interacts with the gas molecules, causing them to absorb energy and emit sound waves.
- passes through the cell containing the gas to be analyzed. The absorption of radiation by water vapor generates a pressure wave (sound) detected by microphones.
- **Differential Helmholtz Resonator**: Two microphones are placed:
  - Microphone A: in the gas flow excited by the laser.
  - Microphone B: in the non-excited gas flow (reference).
- **Differential Subtraction**: The useful signal is obtained by the difference: `Signal = Sound_A - Sound_B`. This amplifies the useful signal and reduces noise (see [Springer](https://link.springer.com/article/10.1007/s00216-019-01877-0), [Université de Reims](https://www.univ-reims.fr/gsma/equipes-de-recherche/physique-moleculaire-et-spectroscopie-ancien/spectrometrie-laser-et-applications/spectrometrie-photoacoustique,22274,37656.html)).

## Signal Processing

1. **Acquisition**: Synchronous recording of signals from both microphones.
2. **Preprocessing**: Digital filtering to eliminate out-of-band noise (band-pass filter around the excitation frequency).
3. **Subtraction**: Calculation of the differential signal (A-B).
4. **Fourier Transform**: Extraction of the amplitude of the fundamental component (laser excitation frequency).
5. **Display/Export**: Output of the amplitude of the useful signal, proportional to the water vapor concentration.

## Approach Advantages

- **Increased sensitivity** thanks to signal amplification in the resonator.
- **Noise reduction** by differential subtraction.
- **Compactness and robustness** of the instrument (see [Photoniques](https://www.photoniques.com/articles/photon/pdf/2011/04/photon201154p39.pdf)).

## Project Structure

```plaintext
rust-photoacoustic/
├── src/
│   ├── lib.rs                 # Library exports
│   ├── main.rs                # Application entry point
│   ├── acquisition/           # Audio signal acquisition module
│   │   └── mod.rs             # Microphone interface using CPAL
│   ├── bin/                   # Binary utilities
│   │   ├── differential.rs    # Differential signal processor utility
│   │   ├── filters.rs         # Audio filter utility
│   │   └── noise_generator.rs # Noise generator utility
│   ├── preprocessing/         # Signal preprocessing module
│   │   ├── mod.rs             # Feature export
│   │   ├── filters.rs         # Digital filters implementation
│   │   ├── filters_test.rs    # Tests for digital filters
│   │   ├── differential.rs    # Differential signal calculation
│   │   └── differential_test.rs # Tests for differential signal
│   ├── spectral/              # Spectral analysis module
│   │   ├── mod.rs             # Feature export
│   │   └── fft.rs             # FFT implementation using rustfft
│   ├── utility/               # Utility functions and tools
│   │   ├── mod.rs             # Module exports
│   │   ├── noise_generator.rs # Noise signal generator
│   │   └── certificate_utilities.rs # SSL certificate utilities
│   └── visualization/         # Visualization module
│       ├── mod.rs             # Module exports and main entry point
│       ├── api.rs             # REST API endpoints using Rocket
│       ├── auth/              # Authentication and authorization system
│       │   ├── mod.rs         # Main auth module
│       │   ├── oauth2/        # OAuth 2.0 implementation
│       │   │   ├── mod.rs     # OAuth2 module exports
│       │   │   ├── handlers.rs # OAuth endpoint handlers
│       │   │   ├── state.rs   # OAuth state management
│       │   │   ├── consent.rs # Consent handling
│       │   │   ├── forms.rs   # Authentication forms
│       │   │   └── auth.rs    # User validation
│       │   ├── jwt/           # JWT token management
│       │   │   ├── mod.rs     # JWT module exports
│       │   │   ├── validator.rs # JWT validation
│       │   │   ├── keys.rs    # Key management
│       │   │   ├── claims.rs  # JWT claims handling
│       │   │   ├── issuer.rs  # Token issuer
│       │   │   ├── token_entry.rs # Token entry types
│       │   │   └── token_map.rs # Token storage
│       │   └── guards/        # Request guards
│       │       ├── mod.rs     # Guards module exports
│       │       └── bearer.rs  # Bearer token guard
│       ├── introspection.rs   # OAuth token introspection
│       ├── oidc.rs           # OpenID Connect discovery
│       ├── server/           # Web server infrastructure
│       │   ├── mod.rs        # Server module exports
│       │   ├── builder.rs    # Rocket server builder
│       │   ├── cors.rs       # CORS configuration
│       │   ├── handlers.rs   # Static content handlers
│       │   └── proxy.rs      # Development proxy
│       ├── pwhash.rs         # Password hashing utilities
│       ├── request_guard.rs  # Connection info guard
│       ├── user_info_reponse.rs # User info response types
│       └── vite_dev_proxy.rs # Vite development proxy
├── web/                       # Frontend SPA
│   ├── src/                   # React components and hooks
│   ├── public/                # Static assets
│   ├── package.json           # NPM dependencies
│   └── vite.config.js         # Vite configuration
├── data/                      # Example data folder
├── examples/                  # Usage examples
├── tests/                     # Integration tests
├── count_lines.sh             # Script to count lines of code
└── Cargo.toml                 # Rust project configuration
```

## Technical Parameters

### Signal Acquisition

- **Sampling rate**: 48 kHz (configurable)
- **Resolution**: 16 bits
- **Channels**: 2 (microphone A and B)
- **Acquisition mode**: synchronous to preserve phase relationships

### Processing

- **Windowing**: Hann or Blackman-Harris (configurable)
- **FFT size**: 4096 points (configurable)
- **Band-pass filter**: 4th order Butterworth, centered on the excitation frequency
- **Averaging**: 10 spectra (configurable)

### The visualization module

The visualization module operates as an embedded web server built with the Rocket framework. It exposes a comprehensive API with OpenAPI documentation automatically generated by the `rocket_openapi` crate.

The server hosts a static single-page application (SPA) developed using Vite and React 19. This SPA presents the analysis results in a clear, user-friendly interface.

#### Architecture Overview

The visualization module is organized into several key components:

- **Authentication System (`auth/`)**: A modular authentication system supporting multiple mechanisms:
  - **OAuth 2.0 (`oauth2/`)**: Complete OAuth2 authorization code flow implementation
  - **JWT Management (`jwt/`)**: Token generation, validation, and key management
  - **Request Guards (`guards/`)**: Rocket guards for API endpoint protection

- **Server Infrastructure (`server/`)**: Core web server components:
  - **Builder**: Rocket server configuration and setup
  - **CORS**: Cross-origin resource sharing configuration
  - **Handlers**: Static content and asset serving
  - **Proxy**: Development-time Vite integration

- **API Layer**: RESTful endpoints for data access and system integration
- **Discovery Services**: OpenID Connect discovery and JWT key set endpoints
- **Introspection**: OAuth 2.0 token introspection for token validation

#### Key Features

- **Modular Authentication**: Clean separation between OAuth2, JWT, and authorization concerns
- **API-first Design**: Enables easy integration with other applications or systems
- **JWT Authentication**: Secure API access with configurable HMAC or RSA signing
- **OAuth 2.0 Compliance**: Standards-compliant authorization server implementation
- **OpenID Connect**: Discovery endpoints for automatic client configuration
- **Modern Frontend**: React 19-based SPA using the [sctg vite react auth0 template](https://github.com/sctg-development/vite-react-heroui-auth0-template)
- **Development Integration**: Seamless Vite development server proxy

#### Authentication Flow

1. **Client Registration**: OAuth2 clients are registered with the authorization server
2. **Authorization Request**: Clients redirect users to the authorization endpoint
3. **User Authentication**: Users authenticate via the login form
4. **Consent**: Users grant or deny permissions for the requested scopes
5. **Token Exchange**: Authorization codes are exchanged for JWT access tokens
6. **API Access**: Clients use Bearer tokens to access protected API endpoints

#### Security Features

- **JWT Token Validation**: Supports both HMAC (HS256) and RSA (RS256) signatures
- **Scope-based Authorization**: Fine-grained permission control
- **Request Guards**: Automatic token validation and user authorization
- **Token Introspection**: RFC 7662 compliant token validation endpoint
- **Secure Defaults**: HTTPS support with automatic certificate generation

This architecture ensures both interactive visualization and secure programmatic access to results, following modern authentication and authorization best practices.

## Available Options

- `--input-device`: Specify the audio input device to use (e.g. `hw:0,0` for ALSA)
- `--input-file`: Specify an audio file to analyze (e.g. `input.wav` must be PCM WAV)
- `--frequency`: Fundamental excitation frequency in Hz (default: 2 kHz)
- `--bandwidth`: Bandwidth of the band-pass filter in Hz (default: 100 Hz)
- `--output`: Output file for results (JSON)
- `--window-size`: Analysis window size in samples (default: 4096)
- `--averages`: Number of spectra to average (default: 10)
- `--server`: Start in server mode (default: true)
- `--web-port`, `-p`: Web server port (default: 8080)
- `--web-address`: Web server address (default: localhost)
- `--hmac-secret`: HMAC secret for JWT signing
- `--config`: Path to configuration file (YAML format)
- `--show-config-schema`: Output the configuration schema as JSON and exit
- `--modbus-enabled`: Enable Modbus functionality
- `--modbus-address`: Modbus server address
- `--modbus-port`: Modbus server port
- `--verbose`, `-v`: Enable verbose logging (debug level)
- `--quiet`, `-q`: Disable all logging output

### Logging Options

The application provides flexible logging control through command line options:

```bash
# Default logging (info level)
cargo run

# Enable verbose logging for debugging
cargo run -- --verbose
# or
cargo run -- -v

# Disable all logging output
cargo run -- --quiet
# or
cargo run -- -q
```

The logging levels are:

- **Default (info)**: Shows general information about application operation
- **Verbose (debug)**: Shows detailed debugging information for troubleshooting
- **Quiet (off)**: Suppresses all log output

Note: If both `--verbose` and `--quiet` are specified, `--quiet` takes precedence and disables all logging.

You can also control logging using the `RUST_LOG` environment variable, which will be respected in addition to these command line options.
