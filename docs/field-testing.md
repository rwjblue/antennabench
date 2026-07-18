# Field Testing And Feedback

AntennaBench field testing is opt-in. Reporters decide what to share, where to
share it, and whether any station or session detail is appropriate to publish.

## Reporting A Finding

Use a public GitHub issue when you are comfortable making the report public. A
useful report can be as small as:

- the AntennaBench version or candidate identifier;
- operating system and architecture;
- the part of the workflow involved;
- what you expected and what happened; and
- reproduction steps you are comfortable sharing.

Callsigns, grids, exact locations, station details, schedules, screenshots,
logs, session bundles, HTML reports, and contact details are optional. They are
never required to report a problem.

If privacy is a concern, contact the maintainer directly using the contact
information on the maintainer's GitHub profile or QRZ listing. Security-sensitive
findings and private station evidence should be sent directly rather than posted
to a public issue.

## Private Material

The reporter chooses both the contact method and the material shared. Private
material is available only to the maintainer and is used only to reproduce and
triage the finding.

Public follow-up issues contain the minimum sanitized facts needed for the work.
The maintainer will not publish a reporter's identity, private evidence,
quotation, or attribution without permission. Private material is deleted when
triage no longer needs it or whenever the reporter asks. If an active
investigation would benefit from longer retention, the maintainer asks first.

A reporter may request deletion or end follow-up at any time.

## Maintainer Triage

Classify feedback as a bug, usability problem, documentation problem,
report/method-comprehension problem, privacy or security concern, support
question, duplicate, or non-actionable feedback. Create a focused sanitized
GitHub issue for actionable work.

Pause the affected test when a finding may:

- lose, duplicate, or silently rewrite experiment evidence;
- confuse planned antenna state with confirmed actual state;
- make interruption or recovery untrustworthy;
- expose private information; or
- encourage an unsupported antenna conclusion.

Field testing adds no telemetry, automatic crash reporting, background upload,
hosted account, or remote-support access. Product feedback is not scientific
evidence about antenna performance.
