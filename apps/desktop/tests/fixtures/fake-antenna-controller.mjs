#!/usr/bin/env node

const [behavior = "exit-zero"] = process.argv.slice(2);

switch (behavior) {
  case "exit-zero":
    process.stdout.write("switched\n");
    break;
  case "exit-nonzero":
    process.stderr.write("rejected\n");
    process.exitCode = 7;
    break;
  case "binary":
    process.stdout.write(Buffer.from([0xff, 0x00, 0x7f]));
    break;
  case "flood":
    process.stdout.write("x".repeat(70 * 1024));
    break;
  case "timeout":
    await new Promise((resolve) => setTimeout(resolve, 5_000));
    break;
  default:
    process.stderr.write(`unknown fake behavior: ${behavior}\n`);
    process.exitCode = 64;
}
