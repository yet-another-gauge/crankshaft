#![no_std]
#![no_main]

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use crankshaft::debug;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::time::{hz, khz};
use embassy_stm32::timer;
use embassy_stm32::timer::pwm_input::PwmInput;
use embassy_stm32::{bind_interrupts, peripherals, Config};
use embassy_time::Timer;

bind_interrupts!(struct Irqs {
    TIM2 => timer::CaptureCompareInterruptHandler<peripherals::TIM2>;
});

// Task for blinking the LED
#[embassy_executor::task]
async fn blink_led(mut led: Output<'static>) {
    loop {
        // Blink the LED to verify timer functionality
        debug!("LED ON");
        led.set_high();
        Timer::after_secs(5).await;

        debug!("LED OFF");
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
    // 2. TIM2 Registers for PWM Input Configuration:
    //
    // TIM2_CR1 (Section 18.4.1):
    // +--------+--------+------+------+--------+------+------+------+------+
    // | 15-10  |  9-8   |  7   |  6-5 |   4    |  3   |  2   |  1   |  0   |
    // +--------+--------+------+------+--------+------+------+------+------+
    // | Res.   | CKD    | ARPE | CMS  | DIR    | OPM  | URS  | UDIS | CEN  |
    // +--------+--------+------+------+--------+------+------+------+------+
    //                                                               ^
    //                                                               |
    //                                                               +-- Set to 1: Counter enabled
    //
    // TIM2_CCMR1 (Capture/Compare Mode Register 1, Section 18.4.7):
    // +--------+--------+--------+--------+--------+--------+
    // | 15-12  | 11-10  |  9-8   |  7-4   |  3-2   |  1-0   |
    // +--------+--------+--------+--------+--------+--------+
    // | IC2F   | IC2PSC |  CC2S  |  IC1F  | IC1PSC |  CC1S  |
    // | [3:0]  | [1:0]  | [1:0]  | [3:0]  | [1:0]  | [1:0]  |
    // +--------+--------+--------+--------+--------+--------+
    //                   ^                          ^
    //                   |                          |
    //                   |                          +-- Set to 01: IC1 mapped on TI1 (rising edge capture)
    //                   +----------------------------- Set to 10: IC2 mapped on TI1 (falling edge capture)

    //
    // TIM2_CCER (Capture/Compare Enable Register, Section 18.4.9):
    // +--------+------+------+------+------+------+------+------+------+
    // | 15-8   |  7   |  6   |  5   |  4   |  3   |  2   |  1   |  0   |
    // +--------+------+------+------+------+------+------+------+------+
    // | Other  | CC2NP| Res. | CC2P | CC2E | CC1NP| Res. | CC1P | CC1E |
    // | Bits   |      |      |      |      |      |      |      |      |
    // +--------+------+------+------+------+------+------+------+------+
    //                        ^      ^                    ^      ^
    //                        |      |                    |      |
    //                        |      |                    |      +-- Set to 1: Capture enabled for CC1
    //                        |      |                    +--------- Set to 0: Capture on rising edge for CC1
    //                        |      +------------------------------ Set to 1: Capture enabled for CC2
    //                        +--------------------------------- --- Set to 1: Capture on falling edge for CC2
    //
    // TIM2_SMCR (Slave Mode Control Register, Section 18.4.3):
    // +------+------+--------+--------+------+------+------+------+
    // |  15  |  14  | 13-12  | 11-8   |  7   | 6-4  |  3   | 2-0  |
    // +------+------+--------+--------+------+------+------+------+
    // | ETP  | ECE  | ETPS   | ETF    | MSM  | TS   | OCCS | SMS  |
    // +------+------+--------+--------+------+------+------+------+
    //                                                      ^
    //                                                      |
    //                                                      +-- Set to 100: Reset Mode - Counter reset on rising edge of TRGI
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
    //   Note: For PWM input from Hall-Effect sensors (see "Speed Sensor Hall-Effect HA-P.pdf"),
    //         the 48 MHz timer clock provides high-resolution pulse width measurements
    // - Communication Peripherals (I2C, SPI, UART): 48 MHz

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

        // Advantages of this configuration:
        // 1. Maximum Performance: Running at the maximum supported frequency (48 MHz)
        // 2. Stable Clock Source: Using the ST-LINK MCO output provides a stable reference
        // 3. Simplified Hardware: No external crystal oscillator required
    }
    let p = embassy_stm32::init(config);

    // Initialize PA5 as output for the onboard LED on the Nucleo F091RC
    let led = Output::new(p.PA5, Level::Low, Speed::Low);

    // Spawn the LED blinking task
    spawner.spawn(blink_led(led)).unwrap();

    // Configure PWM input for Hall-effect sensor
    // The Hall-effect sensor (Speed Sensor Hall-Effect HA-P) has:
    // - Max. frequency: ≤ 10 kHz
    // - Accuracy repeatability of the falling edge: < 1.5% (≤6 kHz), < 2% (≤10 kHz)
    //
    // We'll use a timer frequency of 1 MHz (1 μs resolution) which gives us:
    // - At 6000 RPM (100 Hz): 10,000 timer ticks per revolution
    //   Calculation: 6000 RPM = 100 Hz = 0.01 seconds per revolution = 10,000 μs
    //   With 1 MHz timer (1 μs resolution), we get 10,000 timer ticks per revolution
    //
    // - At max 10 kHz: 100 timer ticks per period
    //   Calculation: 10 kHz = 0.0001 seconds per pulse = 100 μs
    //   With 1 MHz timer, we get 100 timer ticks per pulse at maximum frequency
    //
    // Measurement Limits:
    // - TIM2 on the STM32F091RC is a 32-bit timer, so it can count up to 4,294,967,295 ticks
    // - With 1 MHz timer frequency (1 μs per tick), the maximum measurable period is:
    //   4,294,967,295 μs = 4,294.97 seconds = 71.58 minutes
    // - This corresponds to a minimum measurable frequency of:
    //   1 / 4,294.97 s = 0.000233 Hz
    // - For a 30-tooth crankshaft gear, this means a minimum RPM of:
    //   0.000233 Hz * 60 / 30 = 0.000466 RPM
    //
    // This provides an extremely wide measurement range, from very slow rotations
    // to the sensor's maximum frequency of 10 kHz (600,000 RPM for a single tooth,
    // or 20,000 RPM for a 30-tooth gear).
    let timer_freq = 1_000; // kHz

    // Create and enable PWM input capture on TIM2 using PB3 pin
    // This will capture the Hall-effect sensor signal
    let mut pwm = PwmInput::new_alt(p.TIM2, p.PB3, Pull::None, khz(timer_freq));
    pwm.enable();

    // Main loop for period measurements
    loop {
        // Hall-effect sensor signal timing diagram:
        //
        //     Rising Edge           Falling Edge             Rising Edge
        //         ↓                     ↓                        ↓
        //         ┌─────────────────────┐                        ┌────
        //         │                     │                        │
        // ────────┘                     └────────────────────────┘
        //
        //         |<--- Width Ticks --->|
        //         |<------------- Period Ticks ----------------->|
        //
        // Measurements:
        // - Duty Cycle:   (Width Ticks / Period Ticks) * 100%
        // - Width Ticks:  Timer ticks between rising and falling edge (high state duration)
        // - Period Ticks: Timer ticks between consecutive rising edges (one complete cycle)
        let duty_cycle_percent = pwm.get_duty_cycle();

        let width_ticks = pwm.get_width_ticks();
        let width_us = (width_ticks as u64 * 1_000 / timer_freq as u64) as u32; // milliseconds

        let period_ticks = pwm.get_period_ticks();
        let period_us = (period_ticks as u64 * 1_000 / timer_freq as u64) as u32;
        let period_ms = period_us as f32 / 1_000.0; // milliseconds
        let period_s = period_ms / 1_000.0; // seconds

        // Calculate frequency in Hz (if period is non-zero)
        let frequency_hz = if period_us > 0 { 1.0 / period_s } else { 0.0 };

        // Calculate RPM (revolutions per minute)
        // For a 30-tooth crankshaft gear: RPM = Frequency(Hz) * 60 / 30
        let rpm = frequency_hz * 60.0 / 30.0;

        debug!(
            "Period: {} ticks ({} μs, {} ms, {} s), Width: {} ticks ({} μs), Duty: {}%, Freq: {} Hz, RPM: {}",
            period_ticks, period_us, period_ms, period_s, width_ticks, width_us, duty_cycle_percent, frequency_hz, rpm
        );

        // Small delay to prevent tight looping
        // This doesn't affect timing accuracy since we're using hardware timers
        Timer::after_millis(10).await;
    }
}
