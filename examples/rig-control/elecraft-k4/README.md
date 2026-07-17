# Elecraft K4 Antenna Control Through QK4

These dependency-free Node programs switch and verify a K4/K4D through a local
QK4 CAT endpoint. They default to `127.0.0.1:9299`, execute no shell, and write
small JSON diagnostics for AntennaBench to retain with the invocation record.

The example maps AntennaBench target `1` to KAT4 ANT1 and target `2` to KAT4
ANT2. For receive, it explicitly selects the same KAT4 port for the main
receiver rather than assuming the receiver still follows the transmitter.

| Experiment mode | TX command | Main RX command |
| --- | --- | --- |
| `tx_focused` | `AN1;` or `AN2;` | unchanged |
| `rx_focused` | unchanged | `AR5;` or `AR6;` |
| `whole_station_ab` | `AN1;` or `AN2;` | `AR5;` or `AR6;` |
| `single_antenna_profiling` | `AN1;` or `AN2;` | `AR5;` or `AR6;` |

The switch program sets the required paths and polls CAT read-back. The
verification program performs a new, read-only poll and exits nonzero unless
`AN;` and/or `AR;` report the expected state. AntennaBench still treats the two
program invocations as distinct switch and verification evidence.

## Requirements And Limits

- Node.js 22 or newer.
- QK4 exposing the radio locally at `127.0.0.1:9299`.
- A KAT4 installed and configured for ANT1 and ANT2. Elecraft documents ANT2 as
  unavailable without the ATU.
- HF through 6 meters. `AN` and `AR` do not select transverter antennas.
- The main receiver. This example does not change the K4D sub-receiver antenna.
- Target mappings `A -> 1` and `B -> 2` in the AntennaBench controller profile.

`AR5` and `AR6` mean "ATU RX ANT1" and "ATU RX ANT2". They are not the
receive-only `RX ANT IN1` and `RX ANT IN2` jacks, whose selections are `AR4` and
`AR1` respectively.

## AntennaBench Profile

Use an absolute Node executable path if the desktop app does not inherit the
same `PATH` as your terminal.

Switch command on macOS or Linux:

```text
node /absolute/path/to/antennabench/examples/rig-control/elecraft-k4/switch.mjs --target {target} --mode {mode} --direction {direction}
```

Verification command:

```text
node /absolute/path/to/antennabench/examples/rig-control/elecraft-k4/verify.mjs --target {target} --mode {mode} --direction {direction}
```

The canonical argument arrays are:

```text
switch:
  ["/absolute/path/to/.../switch.mjs", "--target", "{target}", "--mode", "{mode}", "--direction", "{direction}"]

verify:
  ["/absolute/path/to/.../verify.mjs", "--target", "{target}", "--mode", "{mode}", "--direction", "{direction}"]
```

On Windows, select `node.exe` as the program and enter those arguments as
separate ordered values. No quoting or shell interpolation is required in the
canonical array.

The default internal read-back timeout is two seconds and the switch settles
for 150 milliseconds before polling. Optional fixed profile arguments can
override them:

```text
--host 127.0.0.1 --port 9299 --timeout-ms 3000 --settle-ms 250
```

Keep AntennaBench's outer process timeout longer than `--timeout-ms`.

## Manual Test

With QK4 and the radio running, these commands select and then independently
verify ANT1 for a whole-station receive intention:

```bash
node examples/rig-control/elecraft-k4/switch.mjs \
  --target 1 --mode whole_station_ab --direction receive
node examples/rig-control/elecraft-k4/verify.mjs \
  --target 1 --mode whole_station_ab --direction receive
```

Use `--target 2` to select ANT2. A successful command prints one JSON line and
exits zero. A connection failure, CAT rejection, unexpected read-back, invalid
mode/direction combination, or timeout prints JSON to stderr and exits nonzero.

## Command Reference

This example follows the Elecraft K4 Programmer's Reference, revision D12:

- every CAT command ends with `;`;
- `ANn;` selects TX ANT1/2/3 and `AN;` reads it back;
- `AR5;`/`AR6;` select KAT4 ANT1/2 for the main receiver and `AR;` reads it back;
- `AI0;` disables auto-info for each short-lived client so unsolicited state
  does not get mistaken for the requested read-back; and
- a response containing `?` is a rejected command.

The scripts intentionally do not control PTT, transmit enable, frequency,
mode, tuner operation, or any other radio setting.
