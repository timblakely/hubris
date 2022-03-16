// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![no_main]

use drv_spi_api::{self as spi_api, Spi};
use drv_stm32xx_sys_api::{self as sys_api, Sys};
use ecp5::spi::*;
use ecp5::*;
use ringbuf::*;
use userlib::*;

task_slot!(SYS, sys);
task_slot!(SPI, spi_driver);

static COMPRESSED_BITSTREAM: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/ecp5.bin.rle"));

fn load_compressed_bitstream<Ecp5ImplError>(
    ecp5: &Ecp5<Ecp5ImplError>,
    bitstream: &mut &[u8],
) -> Result<(), Ecp5Error<Ecp5ImplError>> {
    let mut decompressor = gnarle::Decompressor::default();
    let mut chunk = [0; 256];

    ecp5.initiate_bitstream_load()?;

    // Clock out the bitstream in 256 byte chunks.
    while !bitstream.is_empty() || !decompressor.is_idle() {
        let out =
            gnarle::decompress(&mut decompressor, bitstream, &mut chunk);
        ecp5.write(out)?;
    }

    ecp5.finalize_bitstream_load()?;
    Ok(())
}

#[export_name = "main"]
fn main() -> ! {
    cfg_if::cfg_if! {
        if #[cfg(target_board = "sidecar-1")] {
            let ecp5_evn = Ecp5Spi {
                sys: Sys::from(SYS.get_task_id()),
                spi: Spi::from(SPI.get_task_id()).device(0),
                done: sys_api::Port::J.pin(15),
                init_n: sys_api::Port::J.pin(12),
                program_n: sys_api::Port::J.pin(13),
            };
            let skip_default_reset_on_boot = false;
        } else if #[cfg(target_board = "gimletlet-2")] {
            let ecp5_evn = Ecp5Spi {
                sys: Sys::from(SYS.get_task_id()),
                spi: Spi::from(SPI.get_task_id()).device(0),
                done: sys_api::Port::E.pin(15),
                init_n: sys_api::Port::D.pin(12),
                program_n: sys_api::Port::B.pin(10),
            };
            let skip_default_reset_on_boot = false;
        } else {
            compile_error!("Board is not supported by the task/ecp5");
        }
    }
    ecp5_evn.configure_gpio();

    let ecp5 = Ecp5::new(&ecp5_evn);

    // Do not reset the device if it is already in UserMode.
    let current_state = ecp5.state().unwrap();

    if !skip_default_reset_on_boot || current_state != DeviceState::UserMode {
        ecp5.reset().unwrap();
        ecp5.id().unwrap();
        load_compressed_bitstream(&ecp5, &mut &*COMPRESSED_BITSTREAM).unwrap();
    }

    let mut incoming = [0u8; 128];
    let mut server = ServerImpl { ecp5 };

    loop {
        idol_runtime::dispatch(&mut incoming, &mut server);
    }
}

struct ServerImpl<'a, Ecp5SpiError> {
    ecp5: Ecp5<'a, Ecp5SpiError>,
}

type RequestError = idol_runtime::RequestError<Ecp5Error<Ecp5SpiError>>;

impl<'a, Ecp5SpiError> InOrderEcp5Impl for ServerImpl<'a, Ecp5SpiError>
where
    idol_runtime::RequestError<Ecp5Error<spi::Ecp5SpiError>>:
        From<Ecp5Error<Ecp5SpiError>>,
{
    fn enable(&mut self, _: &RecvMessage) -> Result<(), RequestError> {
        Ok(self.ecp5.enable()?)
    }

    fn disable(&mut self, _: &RecvMessage) -> Result<(), RequestError> {
        Ok(self.ecp5.disable()?)
    }

    fn state(&mut self, _: &RecvMessage) -> Result<DeviceState, RequestError> {
        Ok(self.ecp5.state()?)
    }

    fn id(&mut self, _: &RecvMessage) -> Result<Id, RequestError> {
        Ok(self.ecp5.id()?)
    }

    fn status(&mut self, _: &RecvMessage) -> Result<u32, RequestError> {
        Ok(self.ecp5.status()?.0)
    }

    fn load_default_bitstream(
        &mut self,
        _: &RecvMessage,
    ) -> Result<(), RequestError> {
        Ok(load_compressed_bitstream(
            &self.ecp5,
            &mut &*COMPRESSED_BITSTREAM,
        )?)
    }
}

include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
