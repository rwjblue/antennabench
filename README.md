# AntennaBench

Run repeatable antenna experiments without losing the story behind the data.

AntennaBench is a local-first desktop app for planning antenna comparisons,
guiding the operator through each change, collecting WSPR observations, and
turning the session into an evidence-focused report. It keeps planned settings,
confirmed actions, missed slots, corrections, and source data distinct so a
clean-looking chart cannot hide a messy experiment.

No account or hosted service is required. Your portable session bundle remains
the source of truth.

## A Session In Four Steps

1. **Plan** — enter station details, name the antennas, and choose their
   repeatable order.
2. **Run** — switch at your own pace; after each antenna is ready, AntennaBench
   selects the next valid WSPR cycle and records how long that antenna was in
   use.
3. **Collect** — optionally receive local WSJT-X decodes; WSPR.live public
   spots are gathered automatically by default. Reverse Beacon Network imports
   support controlled non-WSPR experiments.
4. **Review** — inspect a conservative local report, then export the report or
   the complete session bundle.

AntennaBench shows missing, imbalanced, or conflicting evidence instead of
manufacturing an antenna winner. “Insufficient data” is a useful result when
the session does not support a stronger claim.

## Project Status

AntennaBench is an early preview under active development. There is not yet a
signed end-user release; today it is run from source on macOS.

The desktop workflow can create and reopen sessions, conduct manual/no-rig WSPR
comparisons, collect optional WSJT-X and WSPR.live evidence, import bounded WSPR
and RBN data, render local reports, and export both reports and verified bundle
copies. Rig control, automated winner selection, and hosted report publishing
are not available yet.

## Try The Desktop App

Install Xcode Command Line Tools and [Mise](https://mise.jdx.dev/), then run:

```bash
xcode-select --install
mise install
mise run desktop:dev
```

The first build may take a while. `desktop:dev` launches a local Tauri app; stop
it with Control-C. See [Development](docs/development.md) for tests, supported
tool versions, and the rest of the contributor workflow.

## Learn More

- [Product overview](docs/product.md) explains the intended workflow and the
  evidence standards behind it.
- [Session bundles](docs/bundle-format.md) gives a short, user-facing tour of
  the portable experiment record.
- [Documentation index](docs/README.md) links to architecture, development,
  operations, and technical references.
- [Roadmap](docs/roadmap.md) describes the current direction; GitHub Issues
  track individual unfinished work items.

## License

AntennaBench is licensed under the [Apache License, Version 2.0](LICENSE).
Copyright 2026 Robert Jackson.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in AntennaBench is licensed under the same terms, without
additional terms or conditions.
