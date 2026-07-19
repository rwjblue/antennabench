use antennabench_core::{
    v2::{MutationMember, Provenance, RecordMetaV2},
    RecordSource, SCHEMA_VERSION_V2,
};
use chrono::{DateTime, Utc};

pub(crate) fn record_meta(
    session_id: &str,
    mutation_id: &str,
    member_index: u32,
    member_count: u32,
    recorded_at: DateTime<Utc>,
) -> RecordMetaV2 {
    RecordMetaV2 {
        schema_version: SCHEMA_VERSION_V2,
        session_id: session_id.into(),
        recorded_at,
        provenance: Provenance::from_legacy(RecordSource::WsjtxUdp, env!("CARGO_PKG_VERSION")),
        mutation: MutationMember {
            mutation_id: mutation_id.into(),
            member_index,
            member_count,
        },
        runtime_context_id: None,
    }
}
