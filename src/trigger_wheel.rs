use core::fmt;

#[cfg(feature = "defmt")]
use defmt::Format;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub enum WheelType {
    _36_1,
    _36_2_2_2,
}

impl fmt::Display for WheelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::_36_1 => "36/1",
            Self::_36_2_2_2 => "36/2/2/2",
        };

        write!(f, "{}", message)
    }
}

pub struct TriggerWheel {
    pub wheel_type: WheelType,
}

impl TriggerWheel {
    pub fn new(wheel_type: WheelType) -> Self {
        Self { wheel_type }
    }
}
