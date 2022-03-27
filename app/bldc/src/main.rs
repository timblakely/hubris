#![no_std]
#![no_main]

extern crate panic_itm; // breakpoint on `rust_begin_unwind` to catch panics

// We have to do this if we don't otherwise use it to ensure its vector table
// gets linked in.
extern crate stm32h7;

use stm32h7::stm32h723 as device;

use cortex_m_rt::entry;
use drv_stm32h7_startup::{system_init, ClockConfig};

#[entry]
fn main() -> ! {
    loop {}
}
