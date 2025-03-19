use core::fmt;

#[cfg(feature = "defmt")]
use defmt::Format;

#[cfg_attr(feature = "defmt", derive(Format))]
pub struct TriggerWheel {}

impl TriggerWheel {
    pub fn new() -> Self {
        Self {}
    }
}

impl fmt::Display for TriggerWheel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = "Trigger wheel";
        write!(f, "{}", message)
    }
}
