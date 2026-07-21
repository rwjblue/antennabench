use std::{env, error::Error, io, path::PathBuf};

use antennabench_core::SCHEMA_VERSION_V1;
use antennabench_storage::BundleStore;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let source = args.next().map(PathBuf::from).ok_or_else(usage)?;
    let destination = args.next().map(PathBuf::from).ok_or_else(usage)?;
    if args.next().is_some() {
        return Err(usage().into());
    }

    let mut bundle = BundleStore::new(&source).read_normalized_validated()?;

    // The compatibility projection deliberately retains the measured station,
    // schedule, observations, and derived location fields while dropping live
    // checkpoints, adapter captures, attachments, controller invocations, and
    // runtime diagnostics that are not inputs to the public report.
    bundle.manifest.schema_version = SCHEMA_VERSION_V1;
    bundle.station.schema_version = SCHEMA_VERSION_V1;
    bundle.station.operator_notes = None;
    bundle.antennas.schema_version = SCHEMA_VERSION_V1;
    bundle.schedule.schema_version = SCHEMA_VERSION_V1;
    bundle.analysis.schema_version = SCHEMA_VERSION_V1;
    bundle.analysis.notes.clear();
    for antenna in &mut bundle.antennas.antennas {
        antenna.notes = None;
    }
    for event in &mut bundle.events {
        event.meta.schema_version = SCHEMA_VERSION_V1;
        let public_system_note = event.note.as_deref().is_some_and(|note| {
            note == "Antenna ready for the armed WSPR cycle."
                || note
                    .starts_with("Automatically ended after cumulative WSPR.live capture through ")
        });
        if !public_system_note {
            event.note = None;
        }
    }
    for observation in &mut bundle.observations {
        observation.meta.schema_version = SCHEMA_VERSION_V1;
        observation.raw = serde_json::Value::Null;
    }
    for record in &mut bundle.wsjtx {
        record.meta.schema_version = SCHEMA_VERSION_V1;
        record.raw = serde_json::Value::Null;
    }
    for record in &mut bundle.rig {
        record.meta.schema_version = SCHEMA_VERSION_V1;
        record.raw = serde_json::Value::Null;
    }
    for record in &mut bundle.propagation {
        record.meta.schema_version = SCHEMA_VERSION_V1;
        record.raw = serde_json::Value::Null;
    }

    BundleStore::new(&destination).write(&bundle)?;
    BundleStore::new(&destination).read_normalized_validated()?;
    println!("wrote {}", destination.display());
    Ok(())
}

fn usage() -> io::Error {
    io::Error::other(
        "usage: sanitize_canonical_sample <source.session.antennabundle> <destination.session.wsprabundle>",
    )
}
