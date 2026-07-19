# Attribution And External Data

## Natural Earth Coastline

The report crate embeds a quantized copy of the [Natural Earth 1:110m
coastline](https://www.naturalearthdata.com/downloads/110m-physical-vectors/)
for self-contained static geographic figures. Natural Earth states that its
[raster and vector map data are in the public
domain](https://www.naturalearthdata.com/about/terms-of-use/); inclusion does
not imply endorsement.

The checked-in asset was derived from the Natural Earth vector repository's
`ne_110m_coastline.geojson` source blob
`0dde16b6687ab562b2ca8864bb1b8828b4093c99`, retrieved 2026-07-19. Coordinates
are quantized to tenths of a degree and stored as integer longitude/latitude
pairs without feature metadata. The resulting compile-time asset is 46,306
bytes. A test enforces the owner-approved hard cap of 60 KiB (61,440 bytes), so
future coastline changes cannot silently exceed the report binary-size budget.

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
synthetic as documented in the
[Development Technical Reference](development-reference.md); external service
availability and third-party rights are never treated as bundle invariants.

## NOAA Solar Calculation References

Derived solar context uses the equations documented in NOAA Global Monitoring
Laboratory's [General Solar Position
Calculations](https://gml.noaa.gov/grad/solcalc/solareqns.PDF). Exact twilight
boundaries follow the NOAA/National Weather Service descriptions of [civil,
nautical, and astronomical twilight](https://www.weather.gov/fsd/twilight).
These references define a local deterministic calculation; AntennaBench does
not acquire a NOAA observation, contact a NOAA service while analyzing, or
represent the result as NOAA-provided evidence.
