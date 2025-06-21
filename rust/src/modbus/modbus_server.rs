// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Modbus server implementation for the photoacoustic water vapor analyzer
//!
//! For avoiding confusion with the Modbus master/slave terminology, this module uses
//! the terms "server" and "client" instead. The server is the device that provides data,
//! while the client is the device that requests data.
//!
//! The Modbus master is the device that requests data, while the Modbus slave is the device
//! that provides data. In other words, the Modbus master is here the client and the
//! Modbus slave is here the server.
//!
//! ## Register Map
//!
//! This implementation provides the following Modbus registers:
//!
//! ### Input Registers (Read Only)
//!
//! | Register Address | Description | Unit | Scaling |
//! |-----------------|-------------|------|---------|
//! | 0 | Resonance Frequency | Hz | ×10 (0.1 Hz resolution) |
//! | 1 | Signal Amplitude | - | ×1000 (0.001 resolution) |
//! | 2 | Water Vapor Concentration | ppm | ×10 (0.1 ppm resolution) |
//! | 3 | Measurement Timestamp (Low Word) | epoch seconds | 1 |
//! | 4 | Measurement Timestamp (High Word) | epoch seconds | 1 |
//! | 5 | Status Code | - | 0=normal, 1=warning, 2=error |
//!
//! ### Holding Registers (Read/Write)
//!
//! | Register Address | Description | Unit | Default | Range |
//! |-----------------|-------------|------|---------|-------|
//! | 0 | Measurement Interval | seconds | 10 | 1-3600 |
//! | 1 | Averaging Count | samples | 20 | 1-100 |
//! | 2 | Gain Setting | - | 30 | 0-100 |
//! | 3 | Filter Strength | - | 40 | 0-100 |
//!
//! ## Usage Example
//!
//! See the `examples/modbus_client.rs` file for a complete example of how to use
//! this server with a Modbus client.

use std::{
    collections::HashMap,
    future,
    sync::{Arc, Mutex},
};

use log::{debug, error};

use tokio_modbus::prelude::*;

use crate::processing::computing_nodes::SharedComputingState;
use crate::utility::PhotoacousticDataSource;

/// A Modbus TCP server implementation specific to the photoacoustic water vapor analyzer.
///
/// This server exposes input registers for read-only sensor values (like frequency,
/// amplitude, and concentration) and holding registers for read-write configuration
/// parameters (like measurement interval and gain).
///
/// The server is thread-safe and can handle multiple concurrent client connections.
///
/// ### Register Map
///
/// ## Input Registers (Read-Only)
///
/// These registers contain the latest measurement results:
///
/// - Register 0: Resonance frequency (Hz × 10, 0.1 Hz resolution)
/// - Register 1: Signal amplitude (× 1000, 0.001 resolution)
/// - Register 2: Water vapor concentration (ppm × 10, 0.1 ppm resolution)
/// - Register 3: Timestamp low word (UNIX epoch seconds)
/// - Register 4: Timestamp high word (UNIX epoch seconds)
/// - Register 5: Status code (0=normal, 1=warning, 2=error)
///
/// ## Holding Registers (Read-Write)
///
/// These registers contain configurable parameters:
///
/// - Register 0: Measurement interval (seconds), default: 10
/// - Register 1: Averaging count (samples), default: 20
/// - Register 2: Gain setting, default: 30
/// - Register 3: Filter strength, default: 40
///
/// ### Thread Safety
///
/// All registers are protected with `Mutex` within an `Arc` to allow safe
/// concurrent access from multiple client connections.
pub struct PhotoacousticModbusServer {
    /// Input registers (read-only values like measurements)
    pub input_registers: Arc<Mutex<HashMap<u16, u16>>>,

    /// Holding registers (read-write configuration values)
    pub holding_registers: Arc<Mutex<HashMap<u16, u16>>>,

    /// Reference to shared computing state for real-time data updates
    computing_state: Option<SharedComputingState>,
}

impl tokio_modbus::server::Service for PhotoacousticModbusServer {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = future::Ready<Result<Self::Response, Self::Exception>>;

    /// Process a Modbus request and provide a response
    ///
    /// This method handles different Modbus function codes:
    /// - 0x04: Read Input Registers
    /// - 0x03: Read Holding Registers
    /// - 0x10: Write Multiple Registers
    /// - 0x06: Write Single Register
    ///
    /// Any other function code will return an IllegalFunction exception.
    fn call(&self, req: Self::Request) -> Self::Future {
        debug!("Received Modbus request: {:?}", req);

        // Refresh input registers from computing state before processing read requests
        if matches!(req, Request::ReadInputRegisters(_, _)) {
            self.refresh_from_computing_state();
        }

        let res = match req {
            Request::ReadInputRegisters(addr, cnt) => {
                debug!(
                    "Reading {} input registers starting from address {}",
                    cnt, addr
                );
                register_read(&self.input_registers.lock().unwrap(), addr, cnt)
                    .map(Response::ReadInputRegisters)
            }
            Request::ReadHoldingRegisters(addr, cnt) => {
                debug!(
                    "Reading {} holding registers starting from address {}",
                    cnt, addr
                );
                register_read(&self.holding_registers.lock().unwrap(), addr, cnt)
                    .map(Response::ReadHoldingRegisters)
            }
            Request::WriteMultipleRegisters(addr, values) => {
                debug!(
                    "Writing {} values to holding registers starting from address {}",
                    values.len(),
                    addr
                );
                register_write(&mut self.holding_registers.lock().unwrap(), addr, &values)
                    .map(|_| Response::WriteMultipleRegisters(addr, values.len() as u16))
            }
            Request::WriteSingleRegister(addr, value) => {
                debug!("Writing value {} to holding register {}", value, addr);
                register_write(
                    &mut self.holding_registers.lock().unwrap(),
                    addr,
                    std::slice::from_ref(&value),
                )
                .map(|_| Response::WriteSingleRegister(addr, value))
            }
            _ => {
                error!(
                    "Exception::IllegalFunction - Unimplemented function code in request: {req:?}"
                );
                Err(ExceptionCode::IllegalFunction)
            }
        };

        // Log the result
        if let Err(e) = &res {
            error!("Modbus request error: {:?}", e);
        }

        future::ready(res)
    }
}

impl Default for PhotoacousticModbusServer {
    fn default() -> Self {
        Self::new()
    }
}

impl PhotoacousticModbusServer {
    /// Create a new Modbus server instance with default register values
    ///
    /// This initializes the server with predefined test values:
    ///
    /// ### Input Registers (Read-Only)
    /// - 0: 1234 (Resonance frequency in Hz)
    /// - 1: 5678 (Signal amplitude)
    /// - 2: 1000 (Water vapor concentration in ppm)
    /// - 3 & 4: Current UNIX timestamp
    ///
    /// ### Holding Registers (Read-Write)
    /// - 0: 10 (Measurement interval in seconds)
    /// - 1: 20 (Averaging count in samples)
    /// - 2: 30 (Gain setting)
    /// - 3: 40 (Filter strength)
    ///
    /// ### Returns
    ///
    /// A new `PhotoacousticModbusServer` instance ready to be used with a TCP server.
    pub fn new() -> Self {
        // Initialize input registers with measurement values (with proper scaling)
        let mut input_registers = HashMap::new();
        input_registers.insert(0, 1234 * 10); // Frequency in Hz × 10
        input_registers.insert(1, 5678); // Amplitude × 1000
        input_registers.insert(2, 1000 * 10); // Concentration in ppm × 10

        // Store current timestamp in two 16-bit registers
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        input_registers.insert(3, (now & 0xFFFF) as u16); // Low word
        input_registers.insert(4, ((now >> 16) & 0xFFFF) as u16); // High word

        // Status register - 0 means normal operation
        input_registers.insert(5, 0);

        // Initialize holding registers with configuration values
        let mut holding_registers = HashMap::new();
        holding_registers.insert(0, 10); // Measurement interval (seconds)
        holding_registers.insert(1, 20); // Averaging count (samples)
        holding_registers.insert(2, 30); // Gain setting
        holding_registers.insert(3, 40); // Filter strength

        Self {
            input_registers: Arc::new(Mutex::new(input_registers)),
            holding_registers: Arc::new(Mutex::new(holding_registers)),
            computing_state: None,
        }
    }

    /// Create a new Modbus server instance with a computing state
    ///
    /// Similar to `new()`, but initializes the server with data from the provided
    /// computing state instead of static test values.
    ///
    /// ### Parameters
    ///
    /// * `computing_state` - A shared computing state containing photoacoustic measurements
    ///
    /// ### Returns
    ///
    /// A new `PhotoacousticModbusServer` instance ready to be used with a TCP server.
    pub fn with_computing_state(computing_state: &SharedComputingState) -> Self {
        let mut server = Self::new();

        // Store reference to computing state for live updates
        server.computing_state = Some(Arc::clone(computing_state));

        // Initialize with data from the computing state if available
        server.refresh_from_computing_state();

        server
    }

    /// Update the measurement data in the input registers
    ///
    /// This method allows updating the sensor measurement values that are
    /// exposed through the input registers. The floating-point values are
    /// scaled to fit into 16-bit registers with appropriate precision.
    ///
    /// ### Parameters
    ///
    /// * `frequency` - The resonance frequency (Hz)
    /// * `amplitude` - The signal amplitude
    /// * `concentration` - The water vapor concentration (ppm)
    ///
    /// ### Thread Safety
    ///
    /// This method acquires a lock on the input registers, ensuring thread-safe updates
    /// even when the server is handling client connections.
    ///
    /// ### Value Scaling
    ///
    /// The values are scaled as follows:
    /// * Frequency: multiplied by 10 (0.1 Hz resolution)
    /// * Amplitude: multiplied by 1000 (0.001 resolution)
    /// * Concentration: multiplied by 10 (0.1 ppm resolution)
    pub fn update_measurement_data(&self, frequency: f32, amplitude: f32, concentration: f32) {
        let mut input_regs = self.input_registers.lock().unwrap();

        // Scale and update the registers with the new data
        // For frequency, we want 0.1 Hz resolution, so multiply by 10
        let freq_scaled = (frequency * 10.0).round() as u16;
        input_regs.insert(0, freq_scaled);

        // For amplitude, we want 0.001 resolution, so multiply by 1000
        let amp_scaled = (amplitude * 1000.0).round() as u16;
        input_regs.insert(1, amp_scaled);

        // For concentration, we want 0.1 ppm resolution, so multiply by 10
        let conc_scaled = (concentration * 10.0).round() as u16;
        input_regs.insert(2, conc_scaled);

        // Update the timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        input_regs.insert(3, (now & 0xFFFF) as u16); // Low word
        input_regs.insert(4, ((now >> 16) & 0xFFFF) as u16); // High word

        // Add status register - 0 means normal operation
        if (frequency.is_nan() || amplitude.is_nan() || concentration.is_nan()) {
            input_regs.insert(5, 2); // Error status if any value is NaN
        } else {
            input_regs.insert(5, 0); // Normal operation
        }

        debug!(
            "Updated Modbus input registers with new measurement data: freq={}, amp={}, conc={}",
            frequency, amplitude, concentration
        );
    }

    /// Update measurement data from a computing state
    ///
    /// This method reads the latest values from the shared computing state and
    /// updates the Modbus input registers accordingly.
    ///
    /// ### Parameters
    ///
    /// * `computing_state` - The shared computing state to read from
    ///
    /// ### Returns
    ///
    /// * `true` if the data was successfully updated
    /// * `false` if the computing state could not be read or contained no data
    pub fn update_from_computing_state(&self, computing_state: &SharedComputingState) -> bool {
        if let Ok(state) = computing_state.try_read() {
            // We need at least frequency and amplitude from the peak finder
            if let (Some(frequency), Some(amplitude)) = (state.peak_frequency, state.peak_amplitude)
            {
                // Use concentration if available, otherwise return f32::NAN
                let concentration = state.concentration_ppm.unwrap_or_else(|| f32::NAN);

                self.update_measurement_data(frequency, amplitude, concentration);
                debug!(
                    "Updated Modbus registers from computing state: freq={:.2} Hz, amp={:.2} dB, conc={:.2} ppm",
                    frequency, amplitude, concentration
                );
                return true;
            } else {
                debug!("Computing state does not contain sufficient measurement data (missing frequency or amplitude)");
            }
        } else {
            debug!("Could not read computing state for Modbus update");
        }
        false
    }

    /// Refresh input registers from the stored computing state
    ///
    /// This method is called automatically before processing read requests
    /// to ensure the most up-to-date data is served.
    fn refresh_from_computing_state(&self) {
        if let Some(ref computing_state) = self.computing_state {
            self.update_from_computing_state(computing_state);
        }
    }

    /// Get the current configuration from holding registers
    ///
    /// ### Returns
    ///
    /// A tuple with configuration values:
    /// * Measurement interval (seconds)
    /// * Averaging count (samples)
    /// * Gain setting
    /// * Filter strength
    pub fn get_configuration(&self) -> (u16, u16, u16, u16) {
        let regs = self.holding_registers.lock().unwrap();

        let interval = *regs.get(&0).unwrap_or(&10);
        let averaging = *regs.get(&1).unwrap_or(&20);
        let gain = *regs.get(&2).unwrap_or(&30);
        let filter = *regs.get(&3).unwrap_or(&40);

        (interval, averaging, gain, filter)
    }
}

/// Helper function for reading Modbus registers from a HashMap
///
/// This function handles the process of reading one or more registers
/// from a HashMap-based register collection. It checks if each requested
/// register address exists and returns an error if any address is invalid.
///
/// ### Parameters
///
/// * `registers` - The HashMap containing the register values
/// * `addr` - The starting register address
/// * `cnt` - The number of registers to read
///
/// ### Returns
///
/// * `Result<Vec<u16>, ExceptionCode>` - The register values if successful,
///   or an error code if any address is invalid
///
/// ### Errors
///
/// Returns `ExceptionCode::IllegalDataAddress` if any requested register
/// address does not exist in the HashMap.
fn register_read(
    registers: &HashMap<u16, u16>,
    addr: u16,
    cnt: u16,
) -> Result<Vec<u16>, ExceptionCode> {
    // Preallocate response vector with zeros
    let mut response_values = vec![0; cnt.into()];

    // Check and copy each register value
    for i in 0..cnt {
        let reg_addr = addr + i;
        if let Some(r) = registers.get(&reg_addr) {
            response_values[i as usize] = *r;
        } else {
            error!(
                "Exception::IllegalDataAddress - Register {} not found",
                reg_addr
            );
            return Err(ExceptionCode::IllegalDataAddress);
        }
    }

    debug!("Successfully read {} registers from address {}", cnt, addr);
    Ok(response_values)
}

/// Helper function for writing values to Modbus registers
///
/// This function handles the process of writing one or more values
/// to a HashMap-based register collection. It checks if each target
/// register address exists and returns an error if any address is invalid.
///
/// ### Parameters
///
/// * `registers` - The mutable HashMap containing the register values
/// * `addr` - The starting register address
/// * `values` - The slice of values to write
///
/// ### Returns
///
/// * `Result<(), ExceptionCode>` - Success if all values were written,
///   or an error code if any address is invalid
///
/// ### Errors
///
/// Returns `ExceptionCode::IllegalDataAddress` if any target register
/// address does not exist in the HashMap.
fn register_write(
    registers: &mut HashMap<u16, u16>,
    addr: u16,
    values: &[u16],
) -> Result<(), ExceptionCode> {
    // Write each value to its target register
    for (i, value) in values.iter().enumerate() {
        let reg_addr = addr + i as u16;
        if let Some(r) = registers.get_mut(&reg_addr) {
            *r = *value;
            debug!("Written value {} to register {}", value, reg_addr);
        } else {
            error!(
                "Exception::IllegalDataAddress - Register {} not found",
                reg_addr
            );
            return Err(ExceptionCode::IllegalDataAddress);
        }
    }

    debug!(
        "Successfully wrote {} values starting at register {}",
        values.len(),
        addr
    );
    Ok(())
}
