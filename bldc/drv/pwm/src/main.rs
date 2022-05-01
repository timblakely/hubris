#![no_std]
#![no_main]

use drv_pwm_api::PwmError;
use drv_stm32xx_sys_api as sys_api;
// use idl::InOrderPwmImpl;
use idol_runtime::{NotificationHandler, RequestError};
use stm32h7::stm32h735 as device;
#[allow(unused_imports)] // This is needed for panic_handler
use userlib::*;

task_slot!(SYS, sys);

const TIM1_UPD_MASK: u32 = 1 << 0;
const TIM1_TRG_MASK: u32 = 1 << 1;
const TIM1_CC_MASK: u32 = 1 << 2;

fn _configure_pwm(tim: &device::tim1::RegisterBlock) {
    // Stop the timer if it's running for some reason, like if this task crashed. Disable the main
    // output before doing so, otherwise the PWM signal could be stuck in an undesirable state.
    tim.bdtr.modify(|_, w| w.moe().clear_bit());
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    while tim.cr1.read().cen().bit_is_set() {}
    // Center-aligned mode 2: Up/Down and interrupts on up only.
    tim.cr1
        .modify(|_, w| w.dir().up().cms().center_aligned2().ckd().div1());
    // Enable output state low on idle. Also set the master mode so that trgo2 is written based
    // on `tim_oc4refc`
    // Safety: mms2 doesn't have a valid range or enum set. Bits 0b0111 are tim_oc4refc.
    tim.cr2.modify(|_, w| {
        unsafe {
            w.ccpc()
                .clear_bit()
                .ois1()
                .clear_bit()
                .ois2()
                .clear_bit()
                .ois3()
                .clear_bit()
                .ois4()
                .clear_bit()
                // Configure tim_oc4refc to be on ch4. Note that this must be on mms2 for trgo2!
                .mms2()
                .bits(0b0111)
        }
    });
    // Configure output channels to PWM mode 1. Note: OCxM registers are split between the first
    // three bits and the fourth bit. For PWM mode 1 the fourth bit should be zero which is the
    // reset value, but it's good practice to manually set it anyway.
    tim.ccmr1_output().modify(|_, w| {
        w.cc1s()
            .output()
            .oc1m()
            .pwm_mode1()
            .oc1m_3()
            .clear_bit()
            .cc2s()
            .output()
            .oc2m()
            .pwm_mode1()
            .oc2m_3()
            .clear_bit()
    });
    tim.ccmr2_output().modify(|_, w| {
        w.cc3s()
            .output()
            .oc3m()
            .pwm_mode1()
            .oc3m_3()
            .clear_bit()
            .cc4s()
            .output()
            .oc4m()
            .pwm_mode1()
        // TODO(blakely): This isn't exposed in the PAC. Luckily the reset value is 0...
        // .oc4m_3()
        // .clear_bit()
    });
    // Enable channels 1-5. 1-3 are the output pins, channel 4 is used to trigger the current
    // sampling, and 5 is used as the forced deadtime insertion. Set the output polarity to HIGH
    // (rising edge).
    tim.ccer.modify(|_, w| {
        w.cc1e()
            .set_bit()
            .cc1p()
            .clear_bit()
            .cc2e()
            .set_bit()
            .cc2p()
            .clear_bit()
            .cc3e()
            .set_bit()
            .cc3p()
            .clear_bit()
            .cc4e()
            .set_bit()
            .cc4p()
            .clear_bit()
            .cc5e()
            .set_bit()
            .cc5p()
            .clear_bit()
    });
    // 80kHz@260MHz (520MHz SYSCLK) = Prescalar to 0, ARR to 3250
    // Note: the prescalar is 0-indexed; psc=0 implies prescalar = 1.
    tim.psc.write(|w| w.psc().bits(1 - 1));
    tim.arr.write(|w| w.arr().bits(3250));
    // Set repetition counter to 1, since we only want update TIM1 events on only after the full
    // up/down count cycle.
    // Safety: Upstream: needs range to be explicitly set for safety. 16-bit value.
    tim.rcr.write(|w| unsafe { w.rep().bits(1) });
    // Set ccr values to 0 for all three channels.
    tim.ccr1.write(|w| w.ccr().bits(2000));
    tim.ccr2.write(|w| w.ccr().bits(3250));
    tim.ccr3.write(|w| w.ccr().bits(2000));
    // Set ch5 to PWM mode and enable it.
    // PWM mode 1 is 0110, which is spread out over two separate contiguous bit ranges.
    tim.ccmr3_output
        .modify(|_, w| w.oc5m().bits(110).oc5m_3().bit(false));
    // Configure channels 1-3 to be logical AND'd with channel 5. We allow a maximum of 95.01% duty
    // cycle.
    // TODO(blakely): Modify this to allow for 98%?
    // Safety: PAC doesn't have a range spcified for TIM1[CCR]. It's 16 bit, so max is 65535.
    tim.ccr5.modify(|_, w| unsafe {
        w.gc5c1()
            .set_bit()
            .gc5c2()
            .set_bit()
            .gc5c3()
            .set_bit()
            .ccr()
            .bits(3088)
    });
    // Finally, set trgo2 to fire just before the middle of the deadtime midpoint.
    tim.ccr4.modify(|_, w| w.ccr().bits(3248));
}

fn configure_simple(tim: &device::tim1::RegisterBlock) {
    // Stop the timer if it's running for some reason, like if this task crashed. Disable the main
    // output before doing so, otherwise the PWM signal could be stuck in an undesirable state.
    tim.bdtr.modify(|_, w| w.moe().clear_bit());
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    while tim.cr1.read().cen().bit_is_set() {}

    // Center-aligned mode 2: Up/Down and interrupts on up only.
    tim.cr1.modify(|_, w| {
        w.dir()
            .up()
            .cms()
            .edge_aligned()
            .ckd()
            .div1()
            .udis()
            .clear_bit()
            .urs()
            .counter_only()
    });
    // Enable output state low on idle. Also set the master mode so that trgo2 is written based
    // on `tim_oc4refc`
    // Safety: mms2 doesn't have a valid range or enum set. Bits 0b0111 are tim_oc4refc.
    tim.cr2.modify(|_, w| {
        unsafe {
            w.ccpc()
                .clear_bit()
                .ois1()
                .clear_bit()
                .ois2()
                .clear_bit()
                .ois3()
                .clear_bit()
                .ois4()
                .clear_bit()
                // Configure tim_oc4refc to be on ch4. Note that this must be on mms2 for trgo2!
                .mms2()
                .bits(0b0111)
        }
    });

    tim.ccmr1_output().modify(|_, w| {
        w.cc1s().output().oc1m().pwm_mode1().oc1m_3().clear_bit()
    });
    tim.ccer
        .modify(|_, w| w.cc1e().set_bit().cc1p().clear_bit().cc2e().set_bit());

    tim.psc.write(|w| w.psc().bits(1 - 1));
    tim.arr.write(|w| w.arr().bits(6500));

    tim.rcr.write(|w| unsafe { w.rep().bits(0) });
    // Set ccr values to 0 for all three channels.
    tim.ccr1.write(|w| w.ccr().bits(2000));

    // TODO: Remove this temporary debugging.
    // Enable interrupt on tim1.trgo
    tim.cr2.modify(|_, w| w.mms().compare_oc1());
    tim.dier.modify(|_, w| w.uie().enabled());
    // tim.dier.write(|w| w.cc1ie().enabled());
}

struct ServerImpl {
    timer: &'static device::tim1::RegisterBlock,
    bits: u32,
}

impl idl::InOrderPwmImpl for ServerImpl {
    fn set_duty(
        &mut self,
        _: &RecvMessage,
        channel: u8,
        duty: u16,
    ) -> Result<u16, RequestError<PwmError>> {
        match channel {
            1 => self.timer.ccr1.write(|w| w.ccr().bits(duty)),
            2 => self.timer.ccr2.write(|w| w.ccr().bits(duty)),
            3 => self.timer.ccr3.write(|w| w.ccr().bits(duty)),
            _ => (),
        };
        Ok(duty)
    }
}

impl NotificationHandler for ServerImpl {
    fn current_notification_mask(&self) -> u32 {
        TIM1_TRG_MASK | TIM1_CC_MASK | TIM1_UPD_MASK
    }

    fn handle_notification(&mut self, bits: u32) {
        self.timer.sr.write(|w| unsafe { w.bits(0) });
        // Safety: This is just a quick hack to be able to toggle the GPIO ASAP. Write-only, and
        // any/all updates are atomic.
        let gpioe = unsafe { &*device::GPIOE::ptr() };
        gpioe.bsrr.write(|w| w.bs7().set());
        // unsafe {
        //     let gpioe_bsrr: *mut u32 = 0x58021018 as *mut u32;
        //     *gpioe_bsrr = 0b1000000;
        // }
        {
            // use sys_api::*;
            // let sys = Sys::from(SYS.get_task_id());
            // sys.gpio_set(Port::E.pin(8)).unwrap();
            // self.timer.sr.modify(|_, w| w.uif().clear());
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            cortex_m::asm::nop();
            gpioe.bsrr.write(|w| w.br7().reset());
            // unsafe {
            //     let gpioe_bsrr: *mut u32 = 0x58021018 as *mut u32;
            //     *gpioe_bsrr = 0b1000000_0000000000000000;
            // }
            // sys.gpio_reset(Port::E.pin(8)).unwrap();
        }
        unsafe {
            core::ptr::write_volatile(0x58021018 as *mut u32, 1 << 8);
        }
        sys_irq_control(TIM1_UPD_MASK, true);
        unsafe {
            core::ptr::write_volatile(0x58021018 as *mut u32, 1 << (8 + 16));
        }
        // unsafe {
        //     const gpioe_bsrr: *mut u32 = (0x58021000 + 0x18) as *mut u32;
        //     *gpioe_bsrr = 1 << (8 + 16);
        // }
        self.bits = bits;
        unsafe {
            core::ptr::write_volatile(0x58021018 as *mut u32, 1 << (11 + 16));
        }
        // cortex_m::asm::dmb();
        // cortex_m::asm::dsb();
        // unsafe {
        //     const gpioe_bsrr: *mut u32 = (0x58021000 + 0x18) as *mut u32;
        //     *gpioe_bsrr = 1 << (8 + 16);
        // }
        // sys_irq_control(TIM1_TRG_MASK, true);
        // sys_irq_control(TIM1_CC_MASK, true);
    }
}

#[export_name = "main"]
fn main() -> ! {
    let mut incoming = [0u8; idl::INCOMING_SIZE];

    let sys = sys_api::Sys::from(SYS.get_task_id());

    {
        use sys_api::*;
        sys.enable_clock(Peripheral::Tim1);
        sys.enable_clock(Peripheral::GpioE);
        sys.gpio_configure_alternate(
            Port::E.pin(9).and_pin(11).and_pin(13),
            OutputType::PushPull,
            Speed::Medium,
            Pull::None,
            Alternate::AF1,
        )
        .unwrap();
    }

    // TODO: Remove this temporary debugging.
    {
        // Set GPIOE[7,8,11] as bit-bang.
        use sys_api::*;
        sys.gpio_configure_output(
            Port::E.pin(7).and_pin(8).and_pin(11),
            OutputType::PushPull,
            Speed::VeryHigh,
            Pull::None,
        )
        .unwrap();

        // Enable the trigger IRQ.
        sys_irq_control(1, true);
        // sys_irq_control(TIM1_UPD_MASK, true);
        // sys_irq_control(TIM1_TRG_MASK, true);
        sys_irq_control(TIM1_CC_MASK, true);
    }

    // Safety: this is needlessly unsafe in the API. The TIM1 is essentially a static, and we access
    // it through a & reference so aliasing is not a concern. Were it literally a static, we could
    // just reference it.
    let timer = unsafe { &*device::TIM1::ptr() };
    // configure_pwm(&timer);
    configure_simple(&timer);

    // Kick off the timer.
    timer.cr1.modify(|_, w| w.cen().set_bit());
    // Now that the timer has started, enable the main output to allow current on the pins. If we do
    // this before we enable the timer, we have the potential to get into a state where the PWM pins
    // are in an active state but the timer isn't running, potentially drawing tons of current
    // through any high phases to any low phases.
    timer.bdtr.modify(|_, w| w.moe().set_bit());

    let mut serverimpl = ServerImpl { timer, bits: 0 };

    let mut _i = 0;

    loop {
        // let asdf = sys_recv_closed(&mut incoming, TIM1_CC_MASK, TaskId::KERNEL).unwrap();
        // if asdf.operation & 4 > 0 {
        //     _i += asdf.operation;
        // }
        // let timer = unsafe { &*device::TIM1::ptr() };
        // timer
        //     .sr
        //     .modify(|_, w| w.cc1if().clear().uif().clear().tif().clear());
        idol_runtime::dispatch_n(&mut incoming, &mut serverimpl);
        _i += 1;
    }
}

mod idl {
    use super::PwmError;

    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}