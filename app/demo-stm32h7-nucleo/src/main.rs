// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![no_main]

#[cfg(not(any(feature = "panic-itm", feature = "panic-semihosting")))]
compile_error!(
    "Must have either feature panic-itm or panic-semihosting enabled"
);

// Panic behavior controlled by Cargo features:
#[cfg(feature = "panic-itm")]
extern crate panic_itm; // breakpoint on `rust_begin_unwind` to catch panics
#[cfg(feature = "panic-semihosting")]
extern crate panic_semihosting; // requires a debugger

// We have to do this if we don't otherwise use it to ensure its vector table
// gets linked in.
extern crate stm32h7;

cfg_if::cfg_if! {
    if #[cfg(target_board = "nucleo-h723zg")] {
        use stm32h7::stm32h735 as device;
    } else if #[cfg(target_board = "nucleo-h743zi2")] {
        use stm32h7::stm32h743 as device;
    } else if #[cfg(target_board = "nucleo-h753zi")] {
        use stm32h7::stm32h753 as device;
    } else {
        compile_error!("target_board unknown or missing");
    }
}

use cortex_m_rt::entry;
use drv_stm32h7_startup::{system_init, ClockConfig};

#[entry]
fn main() -> ! {
    cfg_if::cfg_if! {
        if #[cfg(any(target_board = "nucleo-h743zi2", target_board = "nucleo-h753zi"))] {
            const CYCLES_PER_MS: u32 = 400_000;
            const CLOCKS: ClockConfig = ClockConfig {
                // The Nucleo board doesn't include an external crystal, so we
                // derive clocks from the HSI64 oscillator.
                source: drv_stm32h7_startup::ClockSource::Hsi64,
                // 64MHz oscillator frequency is outside VCO input range of
                // 2-16, so we use DIVM to divide it by 4 to 16MHz.
                divm: 4,
                // This means the VCO must accept its wider input range:
                vcosel: device::rcc::pllcfgr::PLL1VCOSEL_A::WIDEVCO,
                pllrange: device::rcc::pllcfgr::PLL1RGE_A::RANGE8,
                // DIVN governs the multiplication of the VCO input frequency to
                // produce the intermediate frequency. We want an IF of 800MHz,
                // or a multiplication of 50x.
                //
                // We subtract 1 to get the DIVN value because the PLL
                // effectively adds one to what we write.
                divn: 50 - 1,
                // P is the divisor from the VCO IF to the system frequency. We
                // want 400MHz, so:
                divp: device::rcc::pll1divr::DIVP1_A::DIV2,
                // Q produces kernel clocks; we set it to 200MHz:
                divq: 4 - 1,
                // R is mostly used by the trace unit and we leave it fast:
                divr: 2 - 1,

                // We run the CPU at the full core rate of 400MHz:
                cpu_div: device::rcc::d1cfgr::D1CPRE_A::DIV1,
                // We down-shift the AHB by a factor of 2, to 200MHz, to meet
                // its constraints (Table 122 in datasheet)
                ahb_div: device::rcc::d1cfgr::HPRE_A::DIV2,
                // We configure all APB for 100MHz. These are relative to the
                // AHB frequency.
                apb1_div: device::rcc::d2cfgr::D2PPRE1_A::DIV2,
                apb2_div: device::rcc::d2cfgr::D2PPRE2_A::DIV2,
                apb3_div: device::rcc::d1cfgr::D1PPRE_A::DIV2,
                apb4_div: device::rcc::d3cfgr::D3PPRE_A::DIV2,

                // Flash runs at 200MHz: 2WS, 2 programming cycles. See
                // reference manual Table 13.
                flash_latency: 2,
                flash_write_delay: 2,
            };
        } else if #[cfg(target_board = "nucleo-h723zg")] {
            // The stm32h7 can technically go up to 550MHz, but if we go over 520MHz we have to turn
            // off ECC. So we just "settle" for 520MHz.
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
                // The NUCLEO-H723 _does_ have an external 8MHz crystal, unlike the other h7 boards.
                // Let's use that.
                source: drv_stm32h7_startup::ClockSource::ExternalCrystal,
                // The PLL's input clock range is from 1-16MHz. The 8MHz external crystal is right
                // in the sweet spot, so we don't need to divide it.
                divm: 1,
                // We want to hit 520MHz which is above the 420MHz limit of the low frequency VCO
                // range, so we select the wide range instead.
                vcosel: device::rcc::pllcfgr::PLL1VCOSEL_A::WIDEVCO,
                // The 8MHz external oscillator is right on the cusp of the 4-8 and 8-16MHz.
                // Selecting the higher frequency range just to be safe.
                pllrange: device::rcc::pllcfgr::PLL1RGE_A::RANGE8,
                // DIVN governs the multiplication of the VCO input frequency to produce the
                // intermediate frequency. We want an IF of 520MHz, or a multiplication of 65x
                // (8*65=520).
                //
                // We subtract 1 to get the DIVN value because the PLL effectively adds one to what
                // we write.
                divn: 65 - 1,
                // P is the divisor from the VCO IF to the system frequency. We want 520MHz, so:
                divp: device::rcc::pll1divr::DIVP1_A::DIV1,
                // Q produces kernel clocks; we set it to 130MHz:
                divq: 4 - 1,
                // R is mostly used by the trace unit and we leave it fast at 260MHz, same as AXI:
                divr: 2 - 1,
                // We run the CPU at the full core rate of 520MHz:
                cpu_div: device::rcc::d1cfgr::D1CPRE_A::DIV1,
                // We down-shift the AHB by a factor of 2, to 260MHz, to meet its constraints of a
                // max of 275MHz (Table 12 in datasheet)
                ahb_div: device::rcc::d1cfgr::HPRE_A::DIV2,
                // We configure all APB for 130MHz, under max of 137.5MHz. These are relative to the
                // AHB frequency.
                apb1_div: device::rcc::d2cfgr::D2PPRE1_A::DIV2,
                apb2_div: device::rcc::d2cfgr::D2PPRE2_A::DIV2,
                apb3_div: device::rcc::d1cfgr::D1PPRE_A::DIV2,
                apb4_div: device::rcc::d3cfgr::D3PPRE_A::DIV2,
                // Flash runs at 260MHz: 3WS, 3 programming cycles. See reference manual Table 16.
                flash_latency: 3,
                flash_write_delay: 3,
            };
        } else {
            compile_error!("target_board unknown or missing");
        }
    }

    system_init(CLOCKS);

    // Turn on profiling. We're sneaking around behind the GPIO driver's back
    // for this, but, it's a debug feature.
    //
    // We use the even pins (inside row) of CN8 for this. In order from board
    // north to south,
    // - 2 = PC8 = syscall number [0]
    // - 4 = PC9 = syscall number [1]
    // - 6 = PC10 = syscall number [2]
    // - 8 = PC11 = syscall number [3]
    // - 10 = PC12 = syscall active
    // - 12 = PD2 = pendsv
    // - 14 = PG2 = systick
    // - 16 = PG3 = general isr
    //
    // Using PC8:12 for this because it's available on even pins 2:12 of CN8,
    // labeled SDMMC.
    let rcc = unsafe { &*device::RCC::ptr() };
    #[rustfmt::skip]
    rcc.ahb4enr.modify(|_, w| {
        w.gpiocen().set_bit()
            .gpioden().set_bit()
            .gpiogen().set_bit()
    });
    cortex_m::asm::dmb();
    let gpioc = unsafe { &*device::GPIOC::ptr() };
    #[rustfmt::skip]
    gpioc.moder.modify(|_, w| {
        w.moder8().output()
            .moder9().output()
            .moder10().output()
            .moder11().output()
            .moder12().output()
    });
    let gpiod = unsafe { &*device::GPIOD::ptr() };
    #[rustfmt::skip]
    gpiod.moder.modify(|_, w| {
        w.moder2().output()
    });
    let gpiog = unsafe { &*device::GPIOG::ptr() };
    #[rustfmt::skip]
    gpiog.moder.modify(|_, w| {
        w.moder2().output()
            .moder3().output()
    });

    kern::profiling::configure_events_table(&PROFILING);

    unsafe { kern::startup::start_kernel(CYCLES_PER_MS) }
}

fn syscall_enter(nr: u32) {
    let gpioc = unsafe { &*device::GPIOC::ptr() };
    gpioc.bsrr.write(|w| {
        if nr & 1 != 0 {
            w.bs8().set_bit();
        }
        if nr & 2 != 0 {
            w.bs9().set_bit();
        }
        if nr & 4 != 0 {
            w.bs10().set_bit();
        }
        if nr & 8 != 0 {
            w.bs11().set_bit();
        }
        w.bs12().set_bit();
        w
    });
}

fn syscall_exit() {
    let gpioc = unsafe { &*device::GPIOC::ptr() };
    #[rustfmt::skip]
    gpioc.bsrr.write(|w| {
        w.br8().set_bit()
            .br9().set_bit()
            .br10().set_bit()
            .br11().set_bit()
            .br12().set_bit()
    });
}

fn secondary_syscall_enter() {
    let gpiod = unsafe { &*device::GPIOD::ptr() };
    gpiod.bsrr.write(|w| w.bs2().set_bit());
}

fn secondary_syscall_exit() {
    let gpiod = unsafe { &*device::GPIOD::ptr() };
    gpiod.bsrr.write(|w| w.br2().set_bit());
}

fn isr_enter() {
    let gpiog = unsafe { &*device::GPIOG::ptr() };
    gpiog.bsrr.write(|w| w.bs3().set_bit());
}

fn isr_exit() {
    let gpiog = unsafe { &*device::GPIOG::ptr() };
    gpiog.bsrr.write(|w| w.br3().set_bit());
}

fn timer_isr_enter() {
    let gpiog = unsafe { &*device::GPIOG::ptr() };
    gpiog.bsrr.write(|w| w.bs2().set_bit());
}

fn timer_isr_exit() {
    let gpiog = unsafe { &*device::GPIOG::ptr() };
    gpiog.bsrr.write(|w| w.br2().set_bit());
}

static PROFILING: kern::profiling::EventsTable = kern::profiling::EventsTable {
    syscall_enter,
    syscall_exit,
    secondary_syscall_enter,
    secondary_syscall_exit,
    isr_enter,
    isr_exit,
    timer_isr_enter,
    timer_isr_exit,
};
