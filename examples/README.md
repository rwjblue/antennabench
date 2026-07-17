# AntennaBench Examples

These examples are optional local integrations, not built-in device support.
They are starting points for operators who are comfortable reviewing and running
station-control code.

> [!CAUTION]
> A controller program runs with the same operating-system authority as
> AntennaBench. Review the source, use the narrowest station permissions
> available, and test it manually before attaching it to a live session.

See [Local Antenna Controller Profiles](../docs/antenna-controller-profiles.md)
for the security model, command templates, placeholders, and run workflow.

## Rig Control

- [Elecraft K4 through QK4](rig-control/elecraft-k4/) switches and verifies KAT4
  ANT1/ANT2 for transmit and main-receiver use without a shell.
