#![no_std]
#![no_main]

use drv_clock_inspect_api::ClockError;
use drv_stm32xx_sys_api as sys_api;
use idol_runtime::RequestError;
use stm32h7::stm32h735 as device;
#[allow(unused_imports)] // This is needed for panic_handler
use userlib::*;

task_slot!(SYS, sys);

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

    let sys = sys_api::Sys::from(SYS.get_task_id());
    sys.enable_clock(sys_api::Peripheral::GpioA);
    sys.gpio_configure_alternate(
        sys_api::Port::A.pin(8),
        sys_api::OutputType::PushPull,
        sys_api::Speed::VeryHigh,
        sys_api::Pull::None,
        sys_api::Alternate::AF0,
    )
    .unwrap();

    // TODO: This should probably be a `sys` call, which in turn should actually call RCC.
    // Safety: this is needlessly unsafe in the API. The RCC is essentially a
    // static, and we access it through a & reference so aliasing is not a
    // concern. Were it literally a static, we could just reference it.
    let rcc = unsafe { &*device::RCC::ptr() };
    rcc.cfgr
        .modify(|_, w| w.mco1pre().bits(0b001).mco1().pll1_q());

    loop {
        idol_runtime::dispatch(&mut incoming, &mut serverimpl);
    }
}

mod idl {
    use super::ClockError;

    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}
