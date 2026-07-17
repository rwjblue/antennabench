# Contributing to AntennaBench

Thank you for helping improve AntennaBench. This guide covers the local setup
and repository references needed to build, test, and change the project.

## Set Up and Run

The complete interactive desktop development workflow is supported on macOS 15
or later. Install the Xcode Command Line Tools and [Mise](https://mise.jdx.dev/),
then clone and initialize the repository:

```bash
xcode-select --install
git clone https://github.com/rwjblue/antennabench.git
cd antennabench
mise install
mise run desktop:dev
```

The first setup downloads the pinned tools, and the first build compiles the
application. Stop the development process with Control-C.

## Contributor References

- [Development guide](docs/development.md) covers setup, routine commands,
  repository layout, and contribution expectations.
- [Architecture overview](docs/architecture.md) explains the major components
  and evidence-protection boundaries.
- [Documentation index](docs/README.md) links to the technical and maintainer
  references.

## Contribution Terms

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in AntennaBench is licensed under the same terms as the project,
without additional terms or conditions.
