#![no_std]
#![no_main]

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use crankshaft::info;
use embassy_executor::Spawner;
use embassy_stm32::gpio::Pull;
use embassy_stm32::time::khz;
use embassy_stm32::timer::input_capture::{CapturePin, InputCapture};
use embassy_stm32::timer::{self, Channel};
use embassy_stm32::{bind_interrupts, peripherals};

bind_interrupts!(struct Irqs {
    TIM3 => timer::CaptureCompareInterruptHandler<peripherals::TIM3>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let ch1 = CapturePin::new_ch1(p.PB4, Pull::None);
    let mut ic = InputCapture::new(
        p.TIM3,
        Some(ch1),
        None,
        None,
        None,
        Irqs,
        khz(100),
        Default::default(),
    );

    loop {
        info!("wait for rising edge");
        ic.wait_for_rising_edge(Channel::Ch1).await;

        let capture_value = ic.get_capture_value(Channel::Ch1);
        info!("new capture! {}", capture_value);
    }
}
