# Water Vapor Analyzer by Laser Photoacoustic Spectroscopy

## Project Objective

Develop a Rust program to analyze the concentration of water vapor in air using laser photoacoustic spectroscopy in a differential Helmholtz resonator. The goal is to process the sound signal to extract the amplitude of the fundamental component related to photoacoustic excitation.

## Physical Principle

- **Photoacoustic Spectroscopy**: A laser passes through a cell containing the gas to be analyzed. The absorption of radiation by water vapor generates a pressure wave (sound) detected by microphones.
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
│   ├── main.rs              # Application entry point
│   ├── acquisition/         # Audio signal acquisition module
│   │   └── mod.rs           # Microphone interface
│   ├── preprocessing/       # Signal preprocessing module
│   │   ├── mod.rs           # Feature export
│   │   ├── filters.rs       # Digital filters
│   │   └── differential.rs  # Differential signal calculation
│   ├── spectral/            # Spectral analysis module
│   │   ├── mod.rs           # Feature export
│   │   └── fft.rs           # FFT implementation
│   └── visualization/       # Visualization module
│       └── mod.rs           # Results display
├── data/                    # Example data folder
├── examples/                # Usage examples
├── tests/                   # Integration tests
└── Cargo.toml               # Rust project configuration
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

## Available Options

- `--input-device`: Specify the audio input device
- `--input-file`: Specify an audio file to analyze
- `--frequency`: Fundamental excitation frequency in Hz
- `--bandwidth`: Bandwidth of the band-pass filter in Hz
- `--output`: Output file for results (JSON)
- `--window-size`: Analysis window size in samples
- `--averages`: Number of spectra to average

## References

- [Springer - Diode laser photoacoustic spectroscopy](https://link.springer.com/article/10.1007/s00216-019-01877-0)
- [Université de Reims - Photoacoustic Spectroscopy](https://www.univ-reims.fr/gsma/equipes-de-recherche/physique-moleculaire-et-spectroscopie-ancien/spectrometrie-laser-et-applications/spectrometrie-photoacoustique,22274,37656.html)
- [Photoniques - Photoacoustic Spectroscopy](https://www.photoniques.com/articles/photon/pdf/2011/04/photon201154p39.pdf)
- [Wikipedia - Photoacoustic spectroscopy](https://en.wikipedia.org/wiki/Photoacoustic_spectroscopy)
