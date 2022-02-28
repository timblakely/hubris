// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{mgmt, pins};
use drv_spi_api::Spi;
use drv_stm32h7_eth as eth;
use drv_stm32xx_sys_api::{Alternate, Port, Sys};
use userlib::task_slot;

task_slot!(SPI, spi_driver);

// This system wants to be woken periodically to do logging
pub const WAKE_INTERVAL: Option<u64> = Some(500);

////////////////////////////////////////////////////////////////////////////////

/// Stateless function to configure ethernet pins before the Bsp struct
/// is actually constructed
pub fn configure_ethernet_pins(sys: &Sys) {
    pins::RmiiPins {
        refclk: Port::A.pin(1),
        crs_dv: Port::A.pin(7),
        tx_en: Port::G.pin(11),
        txd0: Port::G.pin(13),
        txd1: Port::G.pin(12),
        rxd0: Port::C.pin(4),
        rxd1: Port::C.pin(5),
        af: Alternate::AF11,
    }
    .configure(sys);

    pins::MdioPins {
        mdio: Port::A.pin(2),
        mdc: Port::C.pin(1),
        af: Alternate::AF11,
    }
    .configure(sys);
}

pub struct Bsp(mgmt::Bsp);

impl Bsp {
    pub fn new(eth: &mut eth::Ethernet, sys: &Sys) -> Self {
        Self(
            mgmt::Config {
                // SP_TO_LDO_PHY2_EN (turns on both P2V5 and P1V0)
                power_en: Some(Port::I.pin(11)),
                power_good: None,
                pll_lock: None,

                // Based on ordering in app.toml
                ksz8463_spi: Spi::from(SPI.get_task_id()).device(0),
                // SP_TO_EPE_RESET_L
                ksz8463_nrst: Port::A.pin(0),
                ksz8463_rst_type: mgmt::Ksz8463ResetSpeed::Normal,

                // SP_TO_PHY2_COMA_MODE_3V3
                vsc85x2_coma_mode: Some(Port::I.pin(15)),
                // SP_TO_PHY2_RESET_3V3_L
                vsc85x2_nrst: Port::I.pin(14),
                vsc85x2_base_port: 0,
            }
            .build(sys, eth),
        )
    }

    pub fn wake(&self, eth: &mut eth::Ethernet) {
        self.0.wake(eth);
    }
}