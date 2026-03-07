use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ExtractionIntent {
    Exists,
    #[default]
    IdsOnly,
    Summary,
    Fields,
    Full,
}

impl ExtractionIntent {
    pub fn estimated_tokens(&self) -> u64 {
        match self {
            Self::Exists => 1,
            Self::IdsOnly => 2,
            Self::Summary => 10,
            Self::Fields => 25,
            Self::Full => 100,
        }
    }
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
    pub fn is_minimal(&self) -> bool {
        matches!(self, Self::Exists | Self::IdsOnly)
    }
}
