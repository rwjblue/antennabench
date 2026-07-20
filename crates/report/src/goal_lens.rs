use antennabench_core::SessionGoal;

use crate::{ReportDistanceBin, ReportGoalLens, ReportQuestionFamily};

pub(crate) fn project_goal_lens(goal: SessionGoal) -> ReportGoalLens {
    use ReportQuestionFamily::{
        CommonOpportunityDetection, GeographicProfile, ObservedReach, Repeatability,
        SharedPathSignal,
    };

    let (priority, emphasized_distance_bins, practical_meaning) = match goal {
        SessionGoal::GeneralCoverage => (
            vec![
                SharedPathSignal,
                CommonOpportunityDetection,
                ObservedReach,
                GeographicProfile,
                Repeatability,
            ],
            Vec::new(),
            "Prioritizes shared-path signal, common listening opportunities, observed reach, distance and bearing, and repeatability. No universal antenna winner is selected.",
        ),
        SessionGoal::Dx => (
            vec![
                GeographicProfile,
                SharedPathSignal,
                CommonOpportunityDetection,
                Repeatability,
                ObservedReach,
            ],
            vec![ReportDistanceBin::Km3000AndAbove],
            "Prioritizes the prespecified DX-oriented distance category, then shared-path signal, common listening opportunities, repeatability, and the complete observed footprint. Nearer evidence remains visible.",
        ),
        SessionGoal::Regional => (
            vec![
                GeographicProfile,
                CommonOpportunityDetection,
                ObservedReach,
                Repeatability,
                SharedPathSignal,
            ],
            vec![
                ReportDistanceBin::Under500Km,
                ReportDistanceBin::Km500To1499,
                ReportDistanceBin::Km1500To2999,
            ],
            "Prioritizes near, regional, and longer observed paths plus bearing, common listening opportunities, reach, and repeatability. DX-oriented evidence remains visible.",
        ),
        SessionGoal::NvisLocal => (
            vec![
                GeographicProfile,
                CommonOpportunityDetection,
                Repeatability,
                ObservedReach,
                SharedPathSignal,
            ],
            vec![ReportDistanceBin::Under500Km],
            "Prioritizes the near / local, NVIS-oriented distance proxy, common listening opportunities, and repeatability. Distance does not establish NVIS propagation, and all farther evidence remains visible.",
        ),
        SessionGoal::WeakSignalReliability => (
            vec![
                CommonOpportunityDetection,
                Repeatability,
                SharedPathSignal,
                ObservedReach,
                GeographicProfile,
            ],
            Vec::new(),
            "Prioritizes one-sided common-opportunity outcomes and repeated path support, followed by shared-path signal and the complete observed footprint. It does not infer an unmeasured decoder floor.",
        ),
        SessionGoal::SingleAntennaProfiling => (
            vec![ObservedReach, GeographicProfile, Repeatability],
            Vec::new(),
            "Prioritizes the recorded antenna's observed footprint and repeatability. Comparative detection and signal-difference questions do not apply.",
        ),
    };

    ReportGoalLens {
        goal,
        priority,
        emphasized_distance_bins,
        practical_meaning: practical_meaning.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_goal_has_a_fixed_unique_priority_contract() {
        for goal in [
            SessionGoal::GeneralCoverage,
            SessionGoal::Dx,
            SessionGoal::Regional,
            SessionGoal::NvisLocal,
            SessionGoal::WeakSignalReliability,
            SessionGoal::SingleAntennaProfiling,
        ] {
            let lens = project_goal_lens(goal);
            assert_eq!(lens.goal, goal);
            assert_eq!(
                lens.priority.len(),
                lens.priority.iter().copied().collect::<BTreeSet<_>>().len()
            );
            assert!(!lens.practical_meaning.is_empty());
        }
    }

    #[test]
    fn prespecified_distance_emphasis_is_goal_driven() {
        assert_eq!(
            project_goal_lens(SessionGoal::Dx).emphasized_distance_bins,
            vec![ReportDistanceBin::Km3000AndAbove]
        );
        assert_eq!(
            project_goal_lens(SessionGoal::NvisLocal).emphasized_distance_bins,
            vec![ReportDistanceBin::Under500Km]
        );
        assert_eq!(
            project_goal_lens(SessionGoal::Regional).emphasized_distance_bins,
            ReportDistanceBin::ALL[..3]
        );
    }
}
