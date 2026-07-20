use crate::{ReportDistanceBin, EARTH_ANTIPODE_DISTANCE_KM};

impl ReportDistanceBin {
    pub const ALL: [Self; 4] = [
        Self::Under500Km,
        Self::Km500To1499,
        Self::Km1500To2999,
        Self::Km3000AndAbove,
    ];

    /// Semantic category boundaries followed by the physical map horizon.
    /// Renderers may transform these radii, but must not substitute different
    /// category boundaries or labels.
    pub const GEOMETRY_OUTER_EDGES_KM: [f64; 4] =
        [500.0, 1_500.0, 3_000.0, EARTH_ANTIPODE_DISTANCE_KM];

    pub fn classify(distance_km: f64) -> Option<Self> {
        if !distance_km.is_finite() || distance_km < 0.0 {
            return None;
        }
        Some(match distance_km {
            value if value < 500.0 => Self::Under500Km,
            value if value < 1_500.0 => Self::Km500To1499,
            value if value < 3_000.0 => Self::Km1500To2999,
            _ => Self::Km3000AndAbove,
        })
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Under500Km => 0,
            Self::Km500To1499 => 1,
            Self::Km1500To2999 => 2,
            Self::Km3000AndAbove => 3,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Under500Km => "Near / local proxy (under 500 km)",
            Self::Km500To1499 => "Regional (500–1499 km)",
            Self::Km1500To2999 => "Longer path (1500–2999 km)",
            Self::Km3000AndAbove => "DX-oriented (3000 km and above)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_have_exact_half_open_boundaries() {
        for (distance, expected) in [
            (0.0, ReportDistanceBin::Under500Km),
            (499.999, ReportDistanceBin::Under500Km),
            (500.0, ReportDistanceBin::Km500To1499),
            (1_499.999, ReportDistanceBin::Km500To1499),
            (1_500.0, ReportDistanceBin::Km1500To2999),
            (2_999.999, ReportDistanceBin::Km1500To2999),
            (3_000.0, ReportDistanceBin::Km3000AndAbove),
            (
                EARTH_ANTIPODE_DISTANCE_KM,
                ReportDistanceBin::Km3000AndAbove,
            ),
        ] {
            assert_eq!(ReportDistanceBin::classify(distance), Some(expected));
        }
        assert_eq!(ReportDistanceBin::classify(-0.1), None);
        assert_eq!(ReportDistanceBin::classify(f64::NAN), None);
        assert_eq!(ReportDistanceBin::classify(f64::INFINITY), None);
    }

    #[test]
    fn semantic_order_labels_and_map_edges_are_one_contract() {
        for (index, category) in ReportDistanceBin::ALL.into_iter().enumerate() {
            assert_eq!(category.index(), index);
            assert!(!category.label().is_empty());
        }
        assert_eq!(
            ReportDistanceBin::GEOMETRY_OUTER_EDGES_KM,
            [500.0, 1_500.0, 3_000.0, EARTH_ANTIPODE_DISTANCE_KM]
        );
    }
}
