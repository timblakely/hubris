#![no_std]
#![no_main]

extern crate panic_itm; // breakpoint on `rust_begin_unwind` to catch panics

// We have to do this if we don't otherwise use it to ensure its vector table
// gets linked in.
extern crate stm32h7;

use stm32h7::stm32h735 as device;

use cortex_m_rt::entry;
use drv_stm32h7_startup::{system_init, ClockConfig};

#[entry]
fn main() -> ! {
    // The stm32h7 can technically go up to 550MHz, but if we go over 520MHz we have to turn off
    // ECC. So we just "settle" for 520MHz.
    const CYCLES_PER_MS: u32 = 520_000;

    // Now we set up the clock config. To hit the peak of 520MHz, we need to:
    //   1. Use VOS0
    //   2. Divide the external clock by 1 (DIVM1)
    //   3. Set the DIVN1 multiplier to 65x
    //   4. PLL's DIVP1 to 1
    //   5. Ensure D1CPRE prescalar remains at 1
    //   6. Set the HPRE prescalar for the hcc to 2 to ensure it's <= 275MHz
    // This is all done within the system_init call below.
    const CLOCKS: ClockConfig = ClockConfig {
        // The NUCLEO-H723 _does_ have an external 8MHz crystal, unlike the other h7 boards. Let's
        // use that.
        source: drv_stm32h7_startup::ClockSource::ExternalCrystal,
        // The PLL's input clock range is from 1-16MHz. The 8MHz external crystal is right in the
        // sweet spot, so we don't need to divide it.
        divm: 1,
        // We want to hit 520MHz which is above the 420MHz limit of the low frequency VCO range, so
        // we select the wide range instead.
        vcosel: device::rcc::pllcfgr::PLL1VCOSEL_A::WIDEVCO,
        // The 8MHz external oscillator is right on the cusp of the 4-8 and 8-16MHz. Selecting the
        // higher frequency range just to be safe.
        pllrange: device::rcc::pllcfgr::PLL1RGE_A::RANGE8,
        // DIVN governs the multiplication of the VCO input frequency to produce the intermediate
        // frequency. We want an IF of 520MHz, or a multiplication of 65x (8*65=520).
        //
        // We subtract 1 to get the DIVN value because the PLL effectively adds one to what we
        // write.
        divn: 65 - 1,
        // P is the divisor from the VCO IF to the system frequency. We want 520MHz, so:
        divp: device::rcc::pll1divr::DIVP1_A::DIV1,
        // Q produces kernel clocks; we set it to 130MHz:
        divq: 4 - 1,
        // R is mostly used by the trace unit and we leave it fast at 260MHz, same as AXI:
        divr: 2 - 1,
        // We run the CPU at the full core rate of 520MHz:
        cpu_div: device::rcc::d1cfgr::D1CPRE_A::DIV1,
        // We down-shift the AHB by a factor of 2, to 260MHz, to meet its constraints of a max of
        // 275MHz (Table 12 in datasheet)
        ahb_div: device::rcc::d1cfgr::HPRE_A::DIV2,
        // We configure all APB for 130MHz, under max of 137.5MHz. These are relative to the AHB
        // frequency.
        apb1_div: device::rcc::d2cfgr::D2PPRE1_A::DIV2,
        apb2_div: device::rcc::d2cfgr::D2PPRE2_A::DIV2,
        apb3_div: device::rcc::d1cfgr::D1PPRE_A::DIV2,
        apb4_div: device::rcc::d3cfgr::D3PPRE_A::DIV2,
        // Flash runs at 260MHz: 3WS, 3 programming cycles. See reference manual Table 16.
        flash_latency: 3,
        flash_write_delay: 3,
    };

    // Fire up the clocks.
    system_init(CLOCKS);

    // And hand control over to the kernel.
    unsafe { kern::startup::start_kernel(CYCLES_PER_MS) }
}
