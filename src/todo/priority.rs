use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Priority levels for todo items.
/// P0 = critical, P1 = high, P2 = medium
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Priority {
    P0, // Critical
    P1, // High
    #[default]
    P2, // Medium
}

impl Priority {
    /// Cycle to next priority: None -> P0 -> P1 -> P2 -> None
    /// This is called on Option<Priority>, see the impl below for that.
    pub fn next(self) -> Option<Priority> {
        match self {
            Priority::P0 => Some(Priority::P1),
            Priority::P1 => Some(Priority::P2),
            Priority::P2 => None,
        }
    }

    /// Convert to string for database storage
    pub fn to_db_str(self) -> Option<String> {
        Some(self.to_string())
    }

    /// Parse from database string
    pub fn from_db_str(s: Option<&str>) -> Option<Priority> {
        s.and_then(|s| s.parse().ok())
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::P0 => write!(f, "P0"),
            Priority::P1 => write!(f, "P1"),
            Priority::P2 => write!(f, "P2"),
        }
    }
}

impl FromStr for Priority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().as_str() {
            "P0" => Ok(Priority::P0),
            "P1" => Ok(Priority::P1),
            "P2" => Ok(Priority::P2),
            _ => Err(format!("Invalid priority: {}", s)),
        }
    }
}

/// Extension trait for Option<Priority> to enable cycling through None
pub trait PriorityCycle {
    /// Cycle through priorities: None -> P0 -> P1 -> P2 -> None
    fn cycle_priority(self) -> Option<Priority>;
}

impl PriorityCycle for Option<Priority> {
    fn cycle_priority(self) -> Option<Priority> {
        match self {
            None => Some(Priority::P0),
            Some(p) => p.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Priority::P0), "P0");
        assert_eq!(format!("{}", Priority::P1), "P1");
        assert_eq!(format!("{}", Priority::P2), "P2");
    }

    #[test]
    fn test_from_str() {
        assert_eq!("P0".parse::<Priority>().unwrap(), Priority::P0);
        assert_eq!("P1".parse::<Priority>().unwrap(), Priority::P1);
        assert_eq!("P2".parse::<Priority>().unwrap(), Priority::P2);
        assert_eq!("p0".parse::<Priority>().unwrap(), Priority::P0);
        assert_eq!("p1".parse::<Priority>().unwrap(), Priority::P1);
        assert_eq!("p2".parse::<Priority>().unwrap(), Priority::P2);
        assert!("invalid".parse::<Priority>().is_err());
    }

    #[test]
    fn test_cycle_priority() {
        let none: Option<Priority> = None;
        let p0 = none.cycle_priority();
        assert_eq!(p0, Some(Priority::P0));

        let p1 = p0.cycle_priority();
        assert_eq!(p1, Some(Priority::P1));

        let p2 = p1.cycle_priority();
        assert_eq!(p2, Some(Priority::P2));

        let back_to_none = p2.cycle_priority();
        assert_eq!(back_to_none, None);
    }

    #[test]
    fn test_to_db_str() {
        assert_eq!(Priority::P0.to_db_str(), Some("P0".to_string()));
        assert_eq!(Priority::P1.to_db_str(), Some("P1".to_string()));
        assert_eq!(Priority::P2.to_db_str(), Some("P2".to_string()));
    }

    #[test]
    fn test_from_db_str() {
        assert_eq!(Priority::from_db_str(Some("P0")), Some(Priority::P0));
        assert_eq!(Priority::from_db_str(Some("P1")), Some(Priority::P1));
        assert_eq!(Priority::from_db_str(Some("P2")), Some(Priority::P2));
        assert_eq!(Priority::from_db_str(None), None);
        assert_eq!(Priority::from_db_str(Some("invalid")), None);
    }

    #[test]
    fn test_default() {
        assert_eq!(Priority::default(), Priority::P2);
    }

    #[test]
    fn test_copy_clone() {
        let p = Priority::P0;
        let p_copy = p;
        let p_clone = p.clone();
        assert_eq!(p, p_copy);
        assert_eq!(p, p_clone);
    }

    #[test]
    fn test_eq() {
        assert_eq!(Priority::P0, Priority::P0);
        assert_ne!(Priority::P0, Priority::P1);
    }
}
