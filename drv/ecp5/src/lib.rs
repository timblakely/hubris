// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

pub mod types;
pub mod spi;

use ringbuf::*;
use userlib::hl::sleep_for;
use zerocopy::{AsBytes, FromBytes};

pub use types::*;
use spi::*;

#[derive(Copy, Clone, Debug, PartialEq)]
enum Trace {
    None,
    Disabled,
    Enabled,
    Command(Command),
    ReadId(u32, Device),
    ReadUserCode(u32),
    ReadStatus(u32),
    CheckBusy(bool),
    Read32(Command, u32),
    StandardBitstreamDetected,
    EncryptedBitstreamDetected,
    BitstreamError(BitstreamError),
}
ringbuf!(Trace, 16, Trace::None);

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u16)]
pub enum Ecp5Error<Ecp5ImplError> {
    Ecp5ImplError(Ecp5ImplError),
    BitstreamError(BitstreamError),
    PortDisabled,
    InvalidMode,
}

impl<Ecp5ImplError> From<Ecp5ImplError> for Ecp5Error<Ecp5ImplError> {
    fn from(e: Ecp5ImplError) -> Self {
        Self::Ecp5ImplError(e)
    }
}

pub trait Ecp5Impl<Ecp5ImplError> {
    /// PROGAM_N interface. This pin acts as a device reset and when asserted
    /// low force to (re)start the bitstream loading process.
    ///
    /// See FPGA-TN-02039-2.0, 4.5.2 for details.
    fn program_n(&self) -> Result<bool, Ecp5ImplError>;
    fn set_program_n(&self, asserted: bool) -> Result<(), Ecp5ImplError>;

    /// INIT_N interface. This pin can be driven after reset/power up to keep
    /// the device from entering Configuration state. As input it signals
    /// Initialization complete or an error occured during bitstream loading.
    ///
    /// See FPGA-TN-02039-2.0, 4.5.3 for details.
    fn init_n(&self) -> Result<bool, Ecp5ImplError>;
    fn set_init_n(&self, asserted: bool) -> Result<(), Ecp5ImplError>;

    /// DONE interface. This pin signals the device is in User Mode. Asserting
    /// the pin keeps the device from entering User Mode after Configuration.
    ///
    /// See FPGA-TN-02039-2.0, 4.5.4 for details.
    fn done(&self) -> Result<bool, Ecp5ImplError>;
    fn set_done(&self, asserted: bool) -> Result<(), Ecp5ImplError>;

    /// A generic interface to send commands and read/write data from a
    /// configuration port. This interface is intended to be somewhat transport
    /// agnostic so either SPI or JTAG could be used if desired.
    fn write_command(&self, c: Command) -> Result<(), Ecp5ImplError>;
    fn read(&self, buf: &mut [u8]) -> Result<(), Ecp5ImplError>;
    fn write(&self, buf: &[u8]) -> Result<(), Ecp5ImplError>;

    /// The command interface may exist on a shared medium such as SPI. The
    /// following primitives allow the upper half of the driver to issue atomic
    /// commands.
    ///
    /// If no lock control of the medium is needed these can be implemented as
    /// no-op.
    fn lock(&self) -> Result<(), Ecp5ImplError>;
    fn release(&self) -> Result<(), Ecp5ImplError>;
}

pub struct Ecp5<'a, Ecp5ImplError> {
    device: &'a dyn Ecp5Impl<Ecp5ImplError>,
}

impl<'a, Ecp5ImplError> Ecp5<'a, Ecp5ImplError> {
    pub fn new(device: &'a dyn Ecp5Impl<Ecp5ImplError>) -> Self {
        Ecp5 { device }
    }

    /// Return the device state based on the current state of its control pins.
    pub fn state(&self) -> Result<DeviceState, Ecp5Error<Ecp5ImplError>> {
        if !self.device.program_n()? {
            Ok(DeviceState::Disabled)
        } else {
            if self.device.done()? {
                Ok(DeviceState::UserMode)
            } else {
                if self.device.init_n()? {
                    Ok(DeviceState::Configuration)
                } else {
                    Ok(DeviceState::InitializationOrConfigurationError)
                }
            }
        }
    }

    pub fn disable(&self) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        self.device.set_program_n(false)?;
        ringbuf_entry!(Trace::Disabled);
        Ok(())
    }

    pub fn enable(&self) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        self.device.set_program_n(true)?;
        ringbuf_entry!(Trace::Enabled);
        sleep_for(50);
        Ok(())
    }

    pub fn reset(&self) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        self.disable()?;
        sleep_for(50);
        self.enable()
    }

    /// Send a command to the device which does not return or require additional
    /// data. FPGA-TN-02039-2.0, 6.2.5 refers to this as a Class C command.
    pub fn send_command(
        &self,
        c: Command,
    ) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        self.device.lock()?;
        self.device.write_command(c)?;
        self.device.release()?;
        ringbuf_entry!(Trace::Command(c));
        Ok(())
    }

    /// Send a command and read back a number of bytes given by the type T. Note
    /// that data is always returned in big endian order.
    pub fn read<T: Default + AsBytes + FromBytes>(
        &self,
        c: Command,
    ) -> Result<T, Ecp5Error<Ecp5ImplError>> {
        let mut buf = T::default();

        self.device.lock()?;
        self.device.write_command(c)?;
        self.device.read(buf.as_bytes_mut())?;
        self.device.release()?;

        Ok(buf)
    }

    pub fn read16(&self, c: Command) -> Result<u16, Ecp5Error<Ecp5ImplError>> {
        Ok(u16::from_be(self.read(c)?))
    }

    pub fn read32(&self, c: Command) -> Result<u32, Ecp5Error<Ecp5ImplError>> {
        let v = u32::from_be(self.read(c)?);

        match c {
            Command::ReadId => {
                ringbuf_entry!(Trace::ReadId(v, Device::from(v)))
            }
            Command::ReadStatus => ringbuf_entry!(Trace::ReadStatus(v)),
            Command::ReadUserCode => ringbuf_entry!(Trace::ReadUserCode(v)),
            _ => ringbuf_entry!(Trace::Read32(c, v)),
        }

        // The configuration port may be disabled and in the case of a ReadStatus or
        // ReadId command this is easily detectable. Since these are mostly likely
        // the first commands to be issued it is useful to have these return an
        // appropriate error.
        if (c == Command::ReadStatus || c == Command::ReadId) && v == 0xffffffff
        {
            return Err(Ecp5Error::PortDisabled);
        }

        Ok(v)
    }

    pub fn write(&self, buf: &[u8]) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        Ok(self.device.write(buf)?)
    }

    /// Read the device ID.
    pub fn id(&self) -> Result<Id, Ecp5Error<Ecp5ImplError>> {
        let id = self.read32(Command::ReadId)?;
        Ok(Id(id, Device::from(id)))
    }

    /// Read the device user code.
    pub fn user_code(&self) -> Result<u32, Ecp5Error<Ecp5ImplError>> {
        self.read32(Command::ReadUserCode)
    }

    /// Read the Status register
    pub fn status(&self) -> Result<Status, Ecp5Error<Ecp5ImplError>> {
        Ok(Status(self.read32(Command::ReadStatus)?))
    }

    pub fn enable_configuration_mode(
        &self,
    ) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        self.send_command(Command::EnableConfigurationMode)
    }

    pub fn disable_configuration_mode(
        &self,
    ) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        self.send_command(Command::DisableConfigurationMode)
    }

    pub fn await_not_busy(
        &self,
        sleep_interval: u64,
    ) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        while self.status()?.busy() {
            sleep_for(sleep_interval);
        }
        Ok(())
    }

    pub fn await_done(
        &self,
        sleep_interval: u64,
    ) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        while !self.status()?.done() {
            sleep_for(sleep_interval);
        }
        Ok(())
    }

    pub fn initiate_bitstream_load(&self) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        // Put device in configuration mode if required.
        if !self.status()?.write_enabled() {
            self.enable_configuration_mode()?;
        }

        if !self.status()?.write_enabled() {
            return Err(Ecp5Error::InvalidMode);
        }

        self.device.lock()?;
        // Use the Impl to write the command and leave the device locked for the
        // byte stream to follow.
        self.device.write_command(Command::BitstreamBurst)?;
        ringbuf_entry!(Trace::Command(Command::BitstreamBurst));

        Ok(())
    }

    pub fn finalize_bitstream_load(&self) -> Result<(), Ecp5Error<Ecp5ImplError>> {
        self.device.release()?;
        self.await_not_busy(10)?;

        // Perform climb-out checklist; determine if the bitstream was accepted
        // and the device is ready for wake up.
        let status = self.status()?;

        if status.encrypt_preamble_detected() {
            ringbuf_entry!(Trace::EncryptedBitstreamDetected);
        }
        if status.standard_preamble_detected() {
            ringbuf_entry!(Trace::StandardBitstreamDetected);
        }

        let error = status.bitstream_error();

        if error != BitstreamError::None {
            // Log and bail. This leaves the device in configuration mode (and
            // the SPI port enabled), allowing the caller to issue a Refresh
            // command and try again if so desired.
            ringbuf_entry!(Trace::BitstreamError(error));
            return Err(Ecp5Error::BitstreamError(error));
        }

        ringbuf_entry!(Trace::BitstreamError(BitstreamError::None));

        // Return to user mode, initiating the control sequence which will start
        // the fabric. Completion of this transition is externally observable
        // with the DONE pin going high.
        self.disable_configuration_mode()?;

        // Unless the port is set to remain enabled through the FAE bits it will
        // be disabled at this point, i.e. performing a read of the ID or Status
        // registers will result in a PortDisabled error.
        Ok(())
    }
}
