# Water Vapor Analyzer by Laser Photoacoustic Spectroscopy

## Development Philosophy & Disclaimer

This project follows a true continuous integration approach where development happens transparently in the main branch. This allows anyone to observe the real-time evolution of a complex scientific application from early stages through completion.

**Current Status:** This project is actively under development and not yet ready for production use. You're seeing the unfiltered development process - including experiments, refactoring, and iterative improvements.

**What This Means:**

- The codebase may be incomplete or contain non-working components
- APIs and architecture may change significantly between commits
- Tests may occasionally fail as new features are integrated
- Documentation is being written alongside the code and evolves with it

Rather than hiding work-in-progress behind feature branches, this approach demonstrates how modern software development progresses through incremental improvements, refactoring, and testing. Feel free to watch, star, or fork this repository to follow its evolution from prototype to working application.

**License Note:** This code is provided as-is, without warranty, under the terms specified in the LICENSE file.

## Configuration

The application can be configured using a YAML configuration file. By default, it looks for `config.yaml` in the current directory.

### Configuration File

You can specify an alternative configuration file using the `--config` command line argument:

```bash
cargo run -- --config /path/to/your/config.yaml
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
│   │   └── noise_generator.rs # Noise signal generator
│   └── visualization/         # Visualization module
│       ├── mod.rs             # Module exports
│       ├── api.rs             # REST API endpoints using Rocket
│       ├── oxide_auth.rs      # Oauth2 authentication using oxide-auth
│       └── server.rs          # Web server configuration serving the SPA and API
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

The visualization module operates as an embedded web server built with the Rocket framework. It exposes a simple API, with OpenAPI documentation automatically generated by the `rocket_openapi` crate.

The server hosts a static single-page application (SPA) developed using Vite and React 19. This SPA presents the analysis results in a clear, user-friendly interface.

Key features:

- **API-first design:** Enables easy integration with other applications or systems, allowing users to access analysis results programmatically.
- **JWT authentication:** The API is secured with JWT authentication, ensuring that only authorized users can access the analysis results.
- **Modern frontend:** The SPA uses the [sctg vite react auth0 template](https://github.com/sctg-development/vite-react-heroui-auth0-template) for rapid development and a polished user experience.

This architecture ensures both interactive visualization and programmatic access to results.
  
### Used libraries

- `cpal`: Audio input/output library for Rust
- `rustfft`: Fast Fourier Transform library

### Evolution

- **Platform Upgrade**: The project is designed to evolve towards deployment on a Raspberry Pi. The next step is to upgrade the signal acquisition system to use two digital microphones connected via the I2S (Inter-IC Sound) protocol.
- **Why I2S?**: I2S enables high-fidelity, synchronized audio data transfer between the microphones and the Raspberry Pi, ensuring both channels are sampled simultaneously. This is crucial for accurate differential measurements, as it eliminates timing mismatches and external noise interference.
- **Expected Benefits**: Using I2S dual-microphone acquisition will provide cleaner, more reliable signals for both the measurement and reference channels, improving the accuracy and robustness of the water vapor analysis.
- **References**: For more details on I2S and its use with Raspberry Pi, see [Wikipedia](https://en.wikipedia.org/wiki/I2S) and this [I2S & Raspberry Pi article](https://protonestiot.medium.com/setting-up-i2s-soundcard-on-raspberry-pi-inmp441-microphone-f0c8fc859b2e).

## Available Options

- `--input-device`: Specify the audio input device to use (e.g. `hw:0,0` for ALSA)
- `--input-file`: Specify an audio file to analyze (e.g. `input.wav` must be PCM WAV)
- `--frequency`: Fundamental excitation frequency in Hz (default: 2 kHz)
- `--bandwidth`: Bandwidth of the band-pass filter in Hz (default: 100 Hz)
- `--output`: Output file for results (JSON)
- `--window-size`: Analysis window size in samples (default: 4096)
- `--averages`: Number of spectra to average (default: 10)

## Noise Generator Utility

The project includes a standalone noise signal generator utility, located at `src/utility/noise_generator.rs` and available as a binary target. This tool is useful for generating synthetic white noise signals for testing and calibration of the signal processing pipeline.

### Usage

You can run the noise generator from the command line:

```bash
cargo run --bin noise_generator [OPTIONS]
```

### Command Line Options

- `--output <FILE>`: Specify the output WAV file for the generated noise (required).
- `--duration <SECONDS>`: Duration of the generated noise in seconds (default: 10).
- `--sample-rate <HZ>`: Sampling rate of the output file (default: 48000).
- `--channels <N>`: Number of audio channels (default: 2).
- `--correlated`: Set to true to use correlations between channels (default is independent)
- `--correlation`: Correlation coefficient between channels (-1.0 to 1.0) (default: 0)
- `--amplitude <VALUE>`: Amplitude of the noise signal (default: 0.5).

Example:

```bash
cargo run --bin noise_generator -- --output test_noise.wav --duration 5 --amplitude 0.3
```

This will generate a 5-second stereo pink noise WAV file named `test_noise.wav` with amplitude 0.3.

## Differential Signal Utility

The project includes a differential signal processing utility, located at `src/bin/differential.rs`. This tool enables you to create differential signals from WAV files in several ways:

1. Process a stereo WAV file to output the difference between left and right channels (L-R or R-L)
2. Process two mono WAV files to output their difference (file1-file2)

### Usage

You can run the differential processor from the command line:

```bash
cargo run --bin differential [OPTIONS]
```

### Command Line Options

- `--input <FILE>`: Input WAV file (stereo for channel differencing, mono for file differencing).
- `--input2 <FILE>`: Second input WAV file (required for file1-minus-file2 mode).
- `--output <FILE>`: Output WAV file (mono).
- `--mode <MODE>`: Differential mode:
  - `left-minus-right` (default): Left minus Right for stereo files
  - `right-minus-left`: Right minus Left for stereo files
  - `file1-minus-file2`: First file minus second file for mono files
- `--gain <VALUE>`: Gain to apply to the output signal (default: 1.0).

Example:

```bash
# Process a stereo file to get L-R
cargo run --bin differential -- --input stereo_file.wav --output lr_diff.wav --mode LeftMinusRleft-minus-right

# Subtract one mono file from another
cargo run --bin differential -- --input first.wav --input2 second.wav --output diff.wav --mode file1-minus-file2 --gain 1.5
```

This utility is particularly useful for testing the differential signal processing algorithms or preparing test files.

## Audio Filter Utility

The project includes a standalone audio filter utility, located at `src/bin/filters.rs`. This tool allows for the application of different digital filters to WAV files for signal processing and analysis.

### Usage

You can run the filter processor from the command line:

```bash
cargo run --bin filters [OPTIONS]
```

### Command Line Options

- `--input`, `-i <FILE>`: Input WAV file.
- `--output`, `-o <FILE>`: Output WAV file.
- `--filter-type`, `-t <TYPE>`: Filter type to apply (default: bandpass):
  - `bandpass`: Bandpass filter around a center frequency
  - `lowpass`: Lowpass filter with specific cutoff
- `--center-freq`, `-f <HZ>`: Center frequency in Hz for bandpass filter (default: 2000.0).
- `--bandwidth`, `-b <HZ>`: Bandwidth in Hz for bandpass filter (default: 100.0).
- `--cutoff-freq`, `-c <HZ>`: Cutoff frequency in Hz for lowpass filter (default: 5000.0).
- `--order`, `-n <ORDER>`: Filter order for bandpass filter, must be even (default: 4).
- `--channel`, `-l <NUMBER>`: Apply filter to specific channel only (default: all channels).
- `--gain`, `-g <VALUE>`: Gain to apply to the output signal (default: 1.0).

### Examples

```bash
# Apply a bandpass filter centered at 1000 Hz with 50 Hz bandwidth
cargo run --bin filters -- -i input.wav -o output.wav -t bandpass -f 1000 -b 50

# Apply a lowpass filter with 3000 Hz cutoff
cargo run --bin filters -- -i input.wav -o output.wav -t lowpass -c 3000

# Filter only the left channel of a stereo file
cargo run --bin filters -- -i stereo.wav -o filtered.wav -l 0

# Use a higher-order filter for sharper cutoff
cargo run --bin filters -- -i input.wav -o output.wav -n 8 -f 2000 -b 200
```

This utility is useful for isolating specific frequency components in your audio signals or removing unwanted noise before analysis.

## Continuous Integration

This project uses GitHub Actions for continuous integration. The CI pipeline automatically builds and tests the code on multiple platforms:

- Windows (x86_64)
- Ubuntu Linux (x86_64 and ARM64)
- macOS (x86_64 and ARM64)

The CI workflow includes:

- Building the project for all target platforms
- Running tests
- Special handling for tests that require specific timeouts (like introspection tests)
- Code quality checks (formatting and linting)

You can see the workflow status in the GitHub repository under the Actions tab.

## Modbus Client Utility

The project includes a Modbus client utility located at `src/bin/modbus_client.rs`. This tool allows you to interact with the Modbus TCP server component of the photoacoustic analyzer, reading sensor values and configuration parameters via the Modbus protocol.

### Usage

You can run the Modbus client from the command line:

```bash
cargo run --bin modbus_client [OPTIONS]
```

### Command Line Options

- `--address <IP>`: The IP address of the Modbus server (default: 127.0.0.1)
- `--port <PORT>`: The port number of the Modbus server (default: 502)
- `--input-register <ADDRESS>`: Starting input register address to read from (default: 0)
- `--quantity <NUMBER>`: Number of registers to read (default: 6)

### Example Usage

```bash
# Read the default registers from a local Modbus server
cargo run --bin modbus_client

# Connect to a remote server on a non-standard port
cargo run --bin modbus_client -- --address 192.168.1.100 --port 1502

# Read a specific range of registers
cargo run --bin modbus_client -- --input-register 2 --quantity 3
```

### Understanding Register Values

The raw register values may need interpretation according to the Modbus server's register map:

- Register 0: Resonance frequency (Hz × 10, divide by 10.0 for actual value)
- Register 1: Signal amplitude (× 1000, divide by 1000.0 for actual value)
- Register 2: Water vapor concentration (ppm × 10, divide by 10.0 for actual value)
- Registers 3-4: Timestamp as low and high words of a 32-bit UNIX timestamp
- Register 5: Status code (0=normal, 1=warning, 2=error)

For more comprehensive interaction with the Modbus server, refer to the example at `examples/modbus_client.rs`.

## Utility Functions

### Certificate Utilities

The project includes a utility module for generating self-signed SSL certificates at `src/utility/certificate_utilities.rs`. This can be used programmatically:

```rust
use rust_photoacoustic::utility::certificate_utilities;

// Generate a self-signed certificate
certificate_utilities::create_self_signed_cert(
    365,                       // Valid for 365 days
    "path/to/cert.pem",       // Certificate output path
    "path/to/cert.key",       // Private key output path
    "localhost",              // Common name for the certificate
    None,                     // Optional key length (default: 2048)
    Some(vec![                // Optional subject alternative names
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string()
    ])
)?;
```

This utility is used by the build process to automatically generate development certificates, but can also be used in your own code when needed.

### Running Tests Locally

To run the tests locally:

```bash
# Run all tests
cargo test

# Run a specific test with output displayed
cargo test --test introspection_test -- --nocapture
```
