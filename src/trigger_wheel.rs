use heapless::HistoryBuffer;

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

#[cfg(feature = "defmt")]
impl Format for TriggerWheel {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "TriggerWheel {{ ticks_count: {} }}", self.ticks_count())
    }
}
