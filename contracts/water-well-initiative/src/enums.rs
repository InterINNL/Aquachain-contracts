use core::fmt;
use cosmwasm_schema::cw_serde;

#[cw_serde]
#[derive(Default, Hash, Eq)]
pub enum ProjectStatus {
    #[default]
    Proposed, // Project created, awaiting admin validation
    Fundraising, // Admin validated, open for donations
    Funded,      // Goal reached, awaiting admin approval to disburse
    Disbursable, // Admin approved disbursal
    Completed,   // Funds disbursed, project done
    Cancelled,   // Optional: project abandoned
}

impl fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ProjectStatus::Proposed => "proposed",
            ProjectStatus::Fundraising => "fundraising",
            ProjectStatus::Funded => "funded",
            ProjectStatus::Disbursable => "disbursable",
            ProjectStatus::Completed => "completed",
            ProjectStatus::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}
