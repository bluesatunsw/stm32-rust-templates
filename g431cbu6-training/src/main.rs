#![no_main]
#![no_std]

use cortex_m_rt::entry;
use stm32g4xx_hal::{
    gpio::GpioExt, pac, prelude::*, pwr::{PwrExt, VoltageScale}, rcc::*,
    time::{ExtU32, RateExtU32}
};

use defmt_rtt as _;
use panic_probe as _;

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = pac::CorePeripherals::take().unwrap();
    let pwr = dp.PWR.constrain().vos(VoltageScale::Range1 { enable_boost: true }).freeze();
    let mut rcc = dp.RCC.freeze(
        Config::pll()
            .pll_cfg(PllConfig {
                mux: PllSrc::HSE(8.MHz()),
                m: PllMDiv::DIV_2,
                n: PllNMul::MUL_32,
                r: Some(PllRDiv::DIV_2),
                q: Some(PllQDiv::DIV_2),
                p: None,
            }),
            pwr
        );

    let gpioc = dp.GPIOC.split(&mut rcc);
    let mut led = gpioc.pc6.into_push_pull_output();
    let mut delay_syst = cp.SYST.delay(&rcc.clocks);

    // blinky!
    loop {
        defmt::println!("Set LED high!");
        led.set_high();
        delay_syst.delay(1.secs());
        defmt::println!("Set LED low!");
        led.set_low();
        delay_syst.delay(1.secs());
    }
}
