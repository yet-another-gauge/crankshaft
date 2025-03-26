#![no_std]
#![no_main]

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use crankshaft::tick::Tick;
use crankshaft::trigger_wheel::TriggerWheel;
use crankshaft::{debug, info};
use embassy_executor::Spawner;
use embassy_stm32::time::{hz, khz, Hertz};
use embassy_stm32::timer::{self, Channel};
use embassy_stm32::{bind_interrupts, peripherals, Config};
use embassy_stm32::{
    gpio::{Level, Output, Pull, Speed},
    timer::{
        input_capture::{CapturePin, InputCapture},
        low_level::CountingMode,
    },
};
use embassy_time::Timer;

// Timer frequency for input capture.
//
// For a crankshaft with 30 teeth:
// - Idle RPM (800 RPM):
//   - 13.3 revolutions per second = 400 teeth per second
//   - 2.5 ms between teeth = 2500 timer ticks per tooth
// - Average RPM (3000 RPM):
//   - 50 revolutions per second = 1500 teeth per second
//   - 0.67 ms between teeth = 670 timer ticks per tooth
// - Maximum RPM (6000 RPM):
//   - 100 revolutions per second = 3000 teeth per second
//   - 0.33 ms between teeth = 330 timer ticks per tooth
const TIMER_FREQ: Hertz = khz(1_000);

#[cfg(feature = "timer-16bit")]
bind_interrupts!(struct Irqs {
    TIM3 => timer::CaptureCompareInterruptHandler<peripherals::TIM3>;
});

#[cfg(feature = "timer-32bit")]
bind_interrupts!(struct Irqs {
    TIM2 => timer::CaptureCompareInterruptHandler<peripherals::TIM2>;
});

#[embassy_executor::task]
async fn blink_led(mut led: Output<'static>) {
    loop {
        debug!("LED on");
        led.set_high();
        Timer::after_secs(5).await;

        debug!("LED off");
        led.set_low();
        Timer::after_secs(5).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // ===================================================================
    // STM32F091RC Clock Configuration for Nucleo-64 Board
    // ===================================================================
    //
    // Hardware Setup:
    // - The ST-LINK debugger MCU provides an 8 MHz clock signal via its MCO pin
    // - This MCO output is connected to the HSE input of the STM32F091RC on the Nucleo-64 board
    // - The connection is direct (no crystal), so we use HSE in "bypass" mode
    //
    // Clock Configuration Registers Diagram:
    //
    // 1. RCC Registers for Clock Configuration:
    //
    // RCC_CR (Clock Control Register):
    // +--------+------+------+-------+------+------+-------+------+---------+
    // | 31-26  |  25  |  24  | 23-20 |  19  |  18  | 17-16 | 15-8 |   7-0   |
    // +--------+------+------+-------+------+------+-------+------+---------+
    // | Res.   | PLL  | PLL  | PLLM  | CSS  | HSE  | HSE   | Res. | HSI     |
    // |        | RDY  | ON   |       | ON   | BYP  | RDY   |      | bits    |
    // +--------+------+------+-------+------+------+-------+------+---------+
    //                 ^                      ^
    //                 |                      |
    //                 |                      +-- Set to 1: HSE bypass mode (external clock from ST-LINK MCO)
    //                 +------------------------- Set to 1: PLL enabled
    //
    // RCC_CFGR (Clock Configuration Register):
    // +-------+-------+-------+-------+-------+-------+-------+------+-------+------+------+------+------+
    // | 31    | 30-28 | 27-24 | 23-22 | 21-18 | 17    | 16-15 | 14   | 13-11 | 10-8 | 7-4  | 3-2  | 1-0  |
    // +-------+-------+-------+-------+-------+-------+-------+------+-------+------+------+------+------+
    // | PLL   | MCO   | MCO   | Res.  | PLL   | PLL   | PLL   | ADC  | Res.  | PPRE | HPRE | SWS  | SW   |
    // | NODIV | PRE   |       |       | MUL   | XTPRE | SRC   | PRE  |       |      |      |      |      |
    // +-------+-------+-------+-------+-------+-------+-------+------+-------+------+------+------+------+
    //                                 ^               ^
    //                                 |               |
    //                                 |               +-- Set to 10:   HSE selected as PLL input
    //                                 +------------------ Set to 0100: PLL input clock x 6 (8 MHz × 6 = 48 MHz)
    //
    // Note: In STM32F09x, the PLL source is selected in RCC_CFGR bits 16-15 (PLLSRC),
    // not in a separate RCC_PLLCFGR register as in some other STM32 families.
    //
    // Clock Tree Overview:
    // 1. Input: ST-LINK MCO (8 MHz) → HSE input in bypass mode
    //    - Section 6.2.1 HSE Clock: valid HSE range for STM32F091RC: 4-32 MHz
    // 2. PLL: 8 MHz × 6 = 48 MHz
    //    - PLL input after prediv: 8 MHz (Valid range: 4-24 MHz, Section 6.2.3)
    //    - PLL multiplier:         6 (Valid range: 2-16, Section 6.2.3)
    //    - PLL output:             48 MHz (Maximum for STM32F091RC)
    // 3. System Clock: 48 MHz from PLL
    //    - Maximum SYSCLK for STM32F091RC: 48 MHz (Section 6.2.1)
    // 4. Bus Clocks:
    //    - SYSCLK/1: AHB  = 48 MHz
    //    - AHB/1:    APB1 = 48 MHz
    //
    // Resulting Peripheral Frequencies:
    // - Core and CPU: 48 MHz
    // - Flash Memory: 48 MHz
    // - GPIO Ports:   48 MHz
    // - Timers:       48 MHz
    // - Peripherals:  48 MHz

    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;

        // MCO (Master Clock Output) Configuration
        // ----------------------------------------
        // The ST-LINK MCU on the Nucleo board outputs a clock signal on the MCO pin
        // This clock is connected to the HSE input of the STM32F091RC
        // The frequency is fixed at 8 MHz and cannot be changed
        config.rcc.hse = Some(Hse {
            // Set HSE frequency to 8 MHz (from ST-LINK MCO)
            freq: hz(8_000_000),
            // Use Bypass mode because we're using an external clock signal,
            // not a crystal oscillator
            mode: HseMode::Bypass,
        });

        // PLL (Phase-Locked Loop) Configuration
        // -------------------------------------
        config.rcc.pll = Some(Pll {
            // Use HSE (the 8 MHz from ST-LINK MCO) as PLL clock source
            src: PllSource::HSE,
            // Predivider for PLL input: 8 MHz / 1 = 8 MHz PLL input
            // Note: For STM32F091RC, the valid PLL input range is 4-24 MHz
            prediv: PllPreDiv::DIV1,
            // PLL multiplier: 8 MHz * 6 = 48 MHz PLL output
            // Note: STM32F091RC max system clock is 48 MHz
            mul: PllMul::MUL6,
        });

        // Clock Distribution Configuration
        // -------------------------------
        // Select PLL as the system clock source
        config.rcc.sys = Sysclk::PLL1_P;
        // AHB (Advanced High-performance Bus) clock = System clock / 1 = 48 MHz
        // This bus connects to the Flash, DMA, and other core peripherals
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        // APB1 (Advanced Peripheral Bus) clock = AHB clock / 1 = 48 MHz
        // This bus connects to most peripherals including timers
        config.rcc.apb1_pre = APBPrescaler::DIV1;
    }

    let p = embassy_stm32::init(config);

    let led = Output::new(p.PA5, Level::Low, Speed::Low);
    spawner.spawn(blink_led(led)).unwrap();

    #[cfg(feature = "timer-16bit")]
    let (mut ic, ch) = {
        let ch1 = CapturePin::new_ch1(p.PB4, Pull::None);
        let ic = InputCapture::new(
            p.TIM3,
            Some(ch1),
            None,
            None,
            None,
            Irqs,
            TIMER_FREQ,
            CountingMode::EdgeAlignedUp,
        );
        (ic, Channel::Ch1)
    };

    #[cfg(feature = "timer-32bit")]
    let (mut ic, ch) = {
        let ch2 = CapturePin::new_ch2(p.PB3, Pull::None);
        let ic = InputCapture::new(
            p.TIM2,
            None,
            Some(ch2),
            None,
            None,
            Irqs,
            TIMER_FREQ,
            CountingMode::EdgeAlignedUp,
        );
        (ic, Channel::Ch2)
    };

    let mut trigger_wheel = TriggerWheel::new();

    loop {
        ic.wait_for_rising_edge(ch).await;

        let captured_value = ic.get_capture_value(ch);
        let tick = Tick::from_ticks(captured_value);

        trigger_wheel.add_tick(tick);

        info!(
            "Captured value {}, tick {}, ticks stored: {}",
            captured_value,
            tick.ticks(),
            trigger_wheel.ticks_count()
        );
    }
}
