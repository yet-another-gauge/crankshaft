use adskalman::{ObservationModel, TransitionModelLinearNoControl};
use heapless::HistoryBuffer;
use nalgebra::{Matrix1, Matrix1x3, Matrix3, Matrix3x1};

#[cfg(feature = "defmt")]
use defmt::Format;

pub struct TriggerWheel {
    ticks: HistoryBuffer<u32, 128>,
}

impl TriggerWheel {
    pub fn new() -> Self {
        Self {
            ticks: HistoryBuffer::new(),
        }
    }

    pub fn add_tick(&mut self, tick: u32) {
        self.ticks.write(tick);
    }

    pub fn ticks_count(&self) -> usize {
        self.ticks.len()
    }
}

impl ObservationModel<f64, nalgebra::U3, nalgebra::U1> for TriggerWheel {
    fn H(&self) -> &Matrix1x3<f64> {
        static H: Matrix1x3<f64> = Matrix1x3::new(0.0, 1.0, 0.0); // we observe only the second element: velocity
        &H
    }

    fn HT(&self) -> &Matrix3x1<f64> {
        // Transpose of H
        static HT: Matrix3x1<f64> = Matrix3x1::new(0.0, 1.0, 0.0);
        &HT
    }

    fn R(&self) -> &Matrix1<f64> {
        static R: Matrix1<f64> = Matrix1::new(10.0); // higher values mean less trust in measurements
        &R
    }
}

impl TransitionModelLinearNoControl<f64, nalgebra::U3> for TriggerWheel {
    fn F(&self) -> &Matrix3<f64> {
        static F: Matrix3<f64> = Matrix3::new(
            1.0, 0.01, 0.00005, // [1   dt  dt * dt / 2 ]
            0.0, 1.00, 0.01000, // [0   1   dt          ]
            0.0, 0.00, 1.00000, // [0   0   1           ]
        );
        &F
    }

    fn FT(&self) -> &Matrix3<f64> {
        // Transpose of F
        static FT: Matrix3<f64> = Matrix3::new(
            1.00000, 0.00, 0.0, // [1           0   0]
            0.01000, 1.00, 0.0, // [dt          1   0]
            0.00005, 0.01, 1.0, // [dt * dt /1  dt  1]
        );
        &FT
    }

    fn Q(&self) -> &Matrix3<f64> {
        static Q: Matrix3<f64> = Matrix3::new(
            0.001, 0.00, 0.0, //
            0.000, 0.01, 0.0, //
            0.000, 0.00, 0.1, //
        );
        &Q
    }
}

#[cfg(feature = "defmt")]
impl Format for TriggerWheel {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "TriggerWheel {{ ticks_count: {} }}", self.ticks_count())
    }
}
