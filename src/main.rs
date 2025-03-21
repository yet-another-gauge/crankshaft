#![no_std]
#![no_main]

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use crankshaft::trigger_wheel::TriggerWheel;
use crankshaft::{debug, info};
use embassy_executor::Spawner;
use embassy_stm32::time::{hz, khz};
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

    // Initialize PA5 as output for the onboard LED on the Nucleo F091RC
    let led = Output::new(p.PA5, Level::Low, Speed::Low);

    // Spawn the LED blinking task
    spawner.spawn(blink_led(led)).unwrap();

    // The Hall-effect sensor (Speed Sensor Hall-Effect HA-P) has:
    // - Max. frequency: ≤ 10 kHz
    // - Accuracy repeatability of the falling edge: < 1.5% (≤ 6 kHz), < 2% (≤ 10 kHz)
    //
    // We'll use a timer frequency of 1 MHz (1 μs resolution) which gives us:
    // - At 6000 RPM (100 Hz): 10,000 timer ticks per revolution
    //   Calculation: 6000 RPM = 100 Hz = 0.01 seconds per revolution = 10,000 μs
    //   With 1 MHz timer (1 μs resolution), we get 10,000 timer ticks per revolution
    //
    // - At max 10 kHz: 100 timer ticks per period
    //   Calculation: 10 kHz = 0.0001 seconds per pulse = 100 μs
    //   With 1 MHz timer, we get 100 timer ticks per pulse at maximum frequency
    let timer_freq = 1_000; // kHz

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
            khz(timer_freq),
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
            khz(timer_freq),
            CountingMode::EdgeAlignedUp,
        );
        (ic, Channel::Ch2)
    };

    let mut trigger_wheel = TriggerWheel::new();

    loop {
        ic.wait_for_rising_edge(ch).await;

        let capture_ticks = ic.get_capture_value(ch);
        trigger_wheel.add_tick(capture_ticks);

        info!(
            "Capture at tick {} μs, ticks stored: {}",
            capture_ticks,
            trigger_wheel.ticks_count()
        );
    }
}
