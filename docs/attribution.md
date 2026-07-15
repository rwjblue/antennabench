# Attribution And External Data

## Reverse Beacon Network

AntennaBench can import daily archive files published by the
[Reverse Beacon Network](https://www.reversebeacon.net/raw_data/). Reverse
Beacon Network and RBN are names used by that independent service; importing a
file does not imply affiliation or endorsement.

The application does not redistribute RBN archives or spot rows. Operators
select files they acquired separately, and AntennaBench preserves the selected
file only inside their local session bundle for provenance and replay. The
repository fixture is wholly synthetic and is modeled on the documented column
shape. Do not commit real archive rows unless their redistribution permission
and required attribution have been confirmed.

The RBN adapter performs no hidden download, dashboard scraping, telnet
connection, or scheduled acquisition. Links are provided so operators can
review the source and its current terms directly.

## WSPR.live And NOAA SWPC

The existing WSPR.live and NOAA SWPC boundaries retain their provider/source
identity in bundle provenance. Their checked-in test fixtures remain reduced or
synthetic as documented in [Development](development.md); external service
availability and third-party rights are never treated as bundle invariants.
