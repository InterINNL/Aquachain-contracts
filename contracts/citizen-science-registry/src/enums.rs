use core::fmt;
use cosmwasm_schema::cw_serde;

#[cw_serde]
#[derive(Default)]
pub enum SensorStatus {
    #[default]
    Proposed,
    Active,
    Inactive,
}

impl fmt::Display for SensorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status_str = match self {
            SensorStatus::Proposed => "Proposed",
            SensorStatus::Active => "Active",
            SensorStatus::Inactive => "Inactive",
        };
        write!(f, "{}", status_str)
    }
}
