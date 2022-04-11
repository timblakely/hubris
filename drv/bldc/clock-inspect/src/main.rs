#![no_std]
#![no_main]

use drv_clock_inspect_api::ClockError;
use idol_runtime::RequestError;
#[allow(unused_imports)] // This is needed for panic_handler
use userlib::*;

struct ServerImpl;

static mut ASDF: bool = false;

impl idl::InOrderClockInspectImpl for ServerImpl {
    fn mco1_en(
        &mut self,
        _: &RecvMessage,
        enable: bool,
    ) -> Result<u8, RequestError<ClockError>> {
        unsafe { ASDF = enable };
        if enable {
            return Ok(7);
        }
        Ok(4)
    }
}

#[export_name = "main"]
fn main() -> ! {
    let mut incoming = [0u8; idl::INCOMING_SIZE];
    let mut serverimpl = ServerImpl;
    loop {
        idol_runtime::dispatch(&mut incoming, &mut serverimpl);
    }
}

mod idl {
    use super::ClockError;

    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}
