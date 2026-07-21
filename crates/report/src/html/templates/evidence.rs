use askama::Template;

use super::super::view::{
    AntennaSectionView, BandSectionView, EvidenceSummaryView, SlotSectionView,
};

#[derive(Template)]
#[template(path = "report/evidence/antenna.html")]
pub(in crate::html) struct AntennaSectionTemplate {
    pub(in crate::html) view: AntennaSectionView,
}

#[derive(Template)]
#[template(path = "report/evidence/band.html")]
pub(in crate::html) struct BandSectionTemplate {
    pub(in crate::html) view: BandSectionView,
}

#[derive(Template)]
#[template(path = "report/evidence/slot.html")]
pub(in crate::html) struct SlotSectionTemplate {
    pub(in crate::html) view: SlotSectionView,
}

#[derive(Template)]
#[template(path = "report/evidence/summary.html")]
pub(in crate::html) struct EvidenceSummaryTemplate {
    pub(in crate::html) view: EvidenceSummaryView,
}
