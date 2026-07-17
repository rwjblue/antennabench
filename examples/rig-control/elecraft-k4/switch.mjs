#!/usr/bin/env node

import { runCli } from "./k4-cat.mjs";

await runCli("switch", process.argv.slice(2));
