use adskalman::{ObservationModel, TransitionModelLinearNoControl};
use embassy_time::{Duration, Instant};
use heapless::HistoryBuffer;
use nalgebra::{Matrix1, Matrix1x3, Matrix3, Matrix3x1, U1, U3};

#[cfg(feature = "defmt")]
use defmt::Format;

// Time step for the Kalman filter state transition model
const DT: f64 = 0.01; // 10 milliseconds
const DT_SQUARED_HALF: f64 = DT * DT * 0.5;

pub struct TriggerWheel<const N: usize> {
    ticks: HistoryBuffer<Instant, N>,
}

impl<const N: usize> TriggerWheel<N> {
    pub fn new() -> Self {
        Self {
            ticks: HistoryBuffer::new(),
        }
    }

    pub fn add_tick(&mut self, tick: &Instant) -> Option<Duration> {
        let interval = self
            .ticks
            .recent()
            .and_then(|recent_tick| match recent_tick {
                _ if recent_tick <= tick => tick.checked_duration_since(*recent_tick),
                _ => recent_tick.checked_duration_since(*tick),
            });

        self.ticks.write(*tick);

        interval
    }

    pub fn ticks_count(&self) -> usize {
        self.ticks.len()
    }
}

impl<const N: usize> ObservationModel<f64, U3, U1> for TriggerWheel<N> {
    fn H(&self) -> &Matrix1x3<f64> {
        static H: Matrix1x3<f64> = Matrix1x3::new(0.0, 1.0, 0.0);
        &H
    }

    fn HT(&self) -> &Matrix3x1<f64> {
        static HT: Matrix3x1<f64> = Matrix3x1::new(0.0, 1.0, 0.0);
        &HT
    }

    fn R(&self) -> &Matrix1<f64> {
        static R: Matrix1<f64> = Matrix1::new(10.0);
        &R
    }
}

impl<const N: usize> TransitionModelLinearNoControl<f64, U3> for TriggerWheel<N> {
    fn F(&self) -> &Matrix3<f64> {
        // State transition matrix for constant acceleration model.
        #[rustfmt::skip]
        static F: Matrix3<f64> = Matrix3::new(
            1.0,  DT, DT_SQUARED_HALF, // [1   dt  dt * dt / 2 ]
            0.0, 1.0, DT,              // [0   1   dt          ]
            0.0, 0.0, 1.0,             // [0   0   1           ]
        );
        &F
    }

    fn FT(&self) -> &Matrix3<f64> {
        // Transpose of the state transition matrix.
        // Used in the Kalman filter equations for covariance propagation:
        // P' = F·P·Fᵀ + Q
        #[rustfmt::skip]
        static FT: Matrix3<f64> = Matrix3::new(
            1.0,            0.0, 0.0, // [1            0   0]
            DT,             1.0, 0.0, // [dt           1   0]
            DT_SQUARED_HALF, DT, 1.0, // [dt * dt / 2  dt  1]
        );
        &FT
    }

    fn Q(&self) -> &Matrix3<f64> {
        // Process noise covariance matrix.
        // Diagonal elements represent uncertainty in the model for each state variable:
        // - Q[0,0] = 0.001: Low noise for angle (position)      - most  predictable
        // - Q[1,1] =  0.01: Medium noise for angular velocity   - less  predictable
        // - Q[2,2] =   0.1: High noise for angular acceleration - least predictable
        //
        // Higher values make the filter more responsive to measurements but noisier.
        // Lower values make the filter smoother but slower to respond to changes.
        // The increasing values (0.001 → 0.01 → 0.1) reflect decreasing confidence
        // in the model as we move from position to velocity to acceleration.
        #[rustfmt::skip]
        static Q: Matrix3<f64> = Matrix3::new(
            0.001,  0.0, 0.0,
              0.0, 0.01, 0.0,
              0.0,  0.0, 0.1,
        );
        &Q
    }
}

#[cfg(feature = "defmt")]
impl<const N: usize> Format for TriggerWheel<N> {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "TriggerWheel {{ ticks_count: {} }}", self.ticks_count())
    }
}
