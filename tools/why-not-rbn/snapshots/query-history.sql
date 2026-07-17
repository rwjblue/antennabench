-- Exact WSPR.live queries used for this snapshot.
-- 24-hour all-HF receiver query
SELECT
  upperUTF8(rx_sign) AS reporter_call,
  upperUTF8(rx_loc) AS reporter_grid,
  any(rx_lat) AS latitude,
  any(rx_lon) AS longitude,
  count() AS spots,
  min(time) AS first_seen,
  max(time) AS last_seen,
  uniqExact(toDate(time)) AS active_days
FROM wspr.rx
WHERE time >= toDateTime('2026-07-16 17:00:00')
  AND time < toDateTime('2026-07-17 17:00:00')
  AND band IN (1,3,5,7,10,14,18,21,24,28,50)
  AND code = 1
  AND rx_sign != ''
  AND rx_loc != ''
  AND rx_lat BETWEEN -90 AND 90
  AND rx_lon BETWEEN -180 AND 180
GROUP BY reporter_call, reporter_grid
ORDER BY reporter_call, reporter_grid
FORMAT CSVWithNames;

-- 72-hour all-HF receiver query
SELECT
  upperUTF8(rx_sign) AS reporter_call,
  upperUTF8(rx_loc) AS reporter_grid,
  any(rx_lat) AS latitude,
  any(rx_lon) AS longitude,
  count() AS spots,
  min(time) AS first_seen,
  max(time) AS last_seen,
  uniqExact(toDate(time)) AS active_days
FROM wspr.rx
WHERE time >= toDateTime('2026-07-14 17:00:00')
  AND time < toDateTime('2026-07-17 17:00:00')
  AND band IN (1,3,5,7,10,14,18,21,24,28,50)
  AND code = 1
  AND rx_sign != ''
  AND rx_loc != ''
  AND rx_lat BETWEEN -90 AND 90
  AND rx_lon BETWEEN -180 AND 180
GROUP BY reporter_call, reporter_grid
ORDER BY reporter_call, reporter_grid
FORMAT CSVWithNames;

-- 168-hour all-HF receiver query
SELECT
  upperUTF8(rx_sign) AS reporter_call,
  upperUTF8(rx_loc) AS reporter_grid,
  any(rx_lat) AS latitude,
  any(rx_lon) AS longitude,
  count() AS spots,
  min(time) AS first_seen,
  max(time) AS last_seen,
  uniqExact(toDate(time)) AS active_days
FROM wspr.rx
WHERE time >= toDateTime('2026-07-10 17:00:00')
  AND time < toDateTime('2026-07-17 17:00:00')
  AND band IN (1,3,5,7,10,14,18,21,24,28,50)
  AND code = 1
  AND rx_sign != ''
  AND rx_loc != ''
  AND rx_lat BETWEEN -90 AND 90
  AND rx_lon BETWEEN -180 AND 180
GROUP BY reporter_call, reporter_grid
ORDER BY reporter_call, reporter_grid
FORMAT CSVWithNames;

-- One bounded per-band query for 40, 20, and 15 meters
SELECT
  band,
  upperUTF8(rx_sign) AS reporter_call,
  upperUTF8(rx_loc) AS reporter_grid,
  any(rx_lat) AS latitude,
  any(rx_lon) AS longitude,
  countIf(time >= toDateTime('2026-07-16 17:00:00')) AS spots_24h,
  countIf(time >= toDateTime('2026-07-14 17:00:00')) AS spots_72h,
  count() AS spots_168h,
  uniqExactIf(toDate(time), time >= toDateTime('2026-07-16 17:00:00')) AS active_days_24h,
  uniqExactIf(toDate(time), time >= toDateTime('2026-07-14 17:00:00')) AS active_days_72h,
  uniqExact(toDate(time)) AS active_days_168h,
  min(time) AS first_seen_168h,
  max(time) AS last_seen_168h
FROM wspr.rx
WHERE time >= toDateTime('2026-07-10 17:00:00')
  AND time < toDateTime('2026-07-17 17:00:00')
  AND band IN (7,14,21)
  AND code = 1
  AND rx_sign != ''
  AND rx_loc != ''
  AND rx_lat BETWEEN -90 AND 90
  AND rx_lon BETWEEN -180 AND 180
GROUP BY band, reporter_call, reporter_grid
ORDER BY band, reporter_call, reporter_grid
FORMAT CSVWithNames;
