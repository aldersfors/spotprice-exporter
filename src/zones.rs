//! Bidding zone registry. Maps friendly names (`SE3`) to the EIC codes the ENTSO-E API expects.
//!
//! Each entry was checked against the live API on 2026-11-18: a day-ahead request returned prices
//! for every code. Names are the usual market names. The timezone is recorded for local-day
//! statistics (backlog S2.2), which need a timezone database to use it.

/// One bidding zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone {
    /// Short market name, for example `SE3`.
    pub name: &'static str,
    /// EIC code used in ENTSO-E API requests.
    pub eic: &'static str,
    /// IANA timezone of the zone's delivery day.
    pub timezone: &'static str,
    /// Known caveats for this zone, empty when there are none.
    pub note: &'static str,
}

const DE_LU_NOTE: &str = "The API returns two series for this zone. The series with the most points is used. Not yet compared with the ENTSO-E web UI.";

/// All supported bidding zones.
pub const ZONES: &[Zone] = &[
    Zone {
        name: "SE1",
        eic: "10Y1001A1001A44P",
        timezone: "Europe/Stockholm",
        note: "",
    },
    Zone {
        name: "SE2",
        eic: "10Y1001A1001A45N",
        timezone: "Europe/Stockholm",
        note: "",
    },
    Zone {
        name: "SE3",
        eic: "10Y1001A1001A46L",
        timezone: "Europe/Stockholm",
        note: "",
    },
    Zone {
        name: "SE4",
        eic: "10Y1001A1001A47J",
        timezone: "Europe/Stockholm",
        note: "",
    },
    Zone {
        name: "NO1",
        eic: "10YNO-1--------2",
        timezone: "Europe/Oslo",
        note: "",
    },
    Zone {
        name: "NO2",
        eic: "10YNO-2--------T",
        timezone: "Europe/Oslo",
        note: "",
    },
    Zone {
        name: "NO3",
        eic: "10YNO-3--------J",
        timezone: "Europe/Oslo",
        note: "",
    },
    Zone {
        name: "NO4",
        eic: "10YNO-4--------9",
        timezone: "Europe/Oslo",
        note: "",
    },
    Zone {
        name: "NO5",
        eic: "10Y1001A1001A48H",
        timezone: "Europe/Oslo",
        note: "",
    },
    Zone {
        name: "DK1",
        eic: "10YDK-1--------W",
        timezone: "Europe/Copenhagen",
        note: "",
    },
    Zone {
        name: "DK2",
        eic: "10YDK-2--------M",
        timezone: "Europe/Copenhagen",
        note: "",
    },
    Zone {
        name: "FI",
        eic: "10YFI-1--------U",
        timezone: "Europe/Helsinki",
        note: "",
    },
    Zone {
        name: "DE-LU",
        eic: "10Y1001A1001A82H",
        timezone: "Europe/Berlin",
        note: DE_LU_NOTE,
    },
];

/// Find a zone by market name (case-insensitive) or by EIC code.
#[must_use]
pub fn lookup(value: &str) -> Option<&'static Zone> {
    let value = value.trim();
    ZONES
        .iter()
        .find(|zone| zone.eic == value || zone.name.eq_ignore_ascii_case(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_has_unique_name_and_eic() {
        for (i, a) in ZONES.iter().enumerate() {
            for b in &ZONES[i + 1..] {
                assert_ne!(a.name, b.name);
                assert_ne!(a.eic, b.eic);
            }
        }
    }

    #[test]
    fn eic_codes_have_the_expected_shape() {
        for zone in ZONES {
            assert_eq!(zone.eic.len(), 16, "{}", zone.name);
        }
    }

    #[test]
    fn lookup_accepts_name_and_eic_in_any_case() {
        assert_eq!(lookup("SE3").map(|z| z.eic), Some("10Y1001A1001A46L"));
        assert_eq!(lookup("se3").map(|z| z.name), Some("SE3"));
        assert_eq!(lookup("10Y1001A1001A46L").map(|z| z.name), Some("SE3"));
        assert_eq!(lookup(" DE-LU ").map(|z| z.name), Some("DE-LU"));
    }

    #[test]
    fn lookup_rejects_unknown_zones() {
        assert!(lookup("SE9").is_none());
        assert!(lookup("10Y1001A1001A99X").is_none());
        assert!(lookup("").is_none());
    }
}
