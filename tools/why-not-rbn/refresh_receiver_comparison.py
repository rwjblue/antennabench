#!/usr/bin/env python3
"""Refresh the WSPR-versus-RBN receiver comparison used by the documentation.

Default operation is offline and rebuilds all charts from the checked-in aggregate
snapshot. Pass --refresh to issue four bounded WSPR.live aggregate queries and one
RBN active-node request before rebuilding.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import html
import io
import json
import math
import os
import re
import sys
import time
import urllib.parse
import urllib.request
from collections import Counter
from pathlib import Path

POPULAR_BANDS = {7: "40m", 14: "20m", 21: "15m"}
ALL_HF_BANDS = (1, 3, 5, 7, 10, 14, 18, 21, 24, 28, 50)
WINDOWS = (24, 72, 168)
WSPR_ENDPOINT = "https://db1.wspr.live/?query="
RBN_ENDPOINT = "https://www.reversebeacon.net/nodes/detail_json.php"
USER_AGENT = "AntennaBench receiver-footprint research/1.0"


def parse_args() -> argparse.Namespace:
    script_dir = Path(__file__).resolve().parent
    default_repo = script_dir.parent.parent
    p = argparse.ArgumentParser()
    p.add_argument("--repo-root", type=Path, default=default_repo)
    p.add_argument("--snapshot-dir", type=Path, default=script_dir / "snapshots")
    p.add_argument("--world-geojson", type=Path, default=script_dir / "world-outline-natural-earth.geojson")
    p.add_argument("--refresh", action="store_true", help="fetch a fresh bounded snapshot")
    p.add_argument("--offline", action="store_true", help="rebuild from the included snapshot (the default)")
    p.add_argument("--end", help="UTC end time, e.g. 2026-07-17T17:00:00Z; defaults to the last whole UTC hour")
    p.add_argument("--cooldown", type=float, default=6.0, help="seconds between WSPR.live requests")
    p.add_argument("--no-png", action="store_true", help="skip optional PNG rendering")
    return p.parse_args()


def utc_end(value: str | None) -> dt.datetime:
    if value:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=dt.timezone.utc)
        return parsed.astimezone(dt.timezone.utc).replace(microsecond=0)
    now = dt.datetime.now(dt.timezone.utc)
    return now.replace(minute=0, second=0, microsecond=0)


def sql_time(value: dt.datetime) -> str:
    return value.astimezone(dt.timezone.utc).strftime("%Y-%m-%d %H:%M:%S")


def http_get(url: str, timeout: int = 180) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "*/*"})
    last_error: Exception | None = None
    for attempt in range(4):
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                return response.read()
        except Exception as exc:  # pragma: no cover - network dependent
            last_error = exc
            if attempt == 3:
                raise
            time.sleep(2 ** attempt)
    raise RuntimeError(last_error)


def wspr_query(query: str) -> str:
    url = WSPR_ENDPOINT + urllib.parse.quote_plus(query)
    return http_get(url).decode("utf-8")


def receiver_query(start: dt.datetime, end: dt.datetime) -> str:
    bands = ",".join(str(v) for v in ALL_HF_BANDS)
    return f"""SELECT
  upperUTF8(rx_sign) AS reporter_call,
  upperUTF8(rx_loc) AS reporter_grid,
  any(rx_lat) AS latitude,
  any(rx_lon) AS longitude,
  count() AS spots,
  min(time) AS first_seen,
  max(time) AS last_seen,
  uniqExact(toDate(time)) AS active_days
FROM wspr.rx
WHERE time >= toDateTime('{sql_time(start)}')
  AND time < toDateTime('{sql_time(end)}')
  AND band IN ({bands})
  AND code = 1
  AND rx_sign != ''
  AND rx_loc != ''
  AND rx_lat BETWEEN -90 AND 90
  AND rx_lon BETWEEN -180 AND 180
GROUP BY reporter_call, reporter_grid
ORDER BY reporter_call, reporter_grid
FORMAT CSVWithNames"""


def band_query(start168: dt.datetime, start72: dt.datetime, start24: dt.datetime, end: dt.datetime) -> str:
    bands = ",".join(str(v) for v in POPULAR_BANDS)
    return f"""SELECT
  band,
  upperUTF8(rx_sign) AS reporter_call,
  upperUTF8(rx_loc) AS reporter_grid,
  any(rx_lat) AS latitude,
  any(rx_lon) AS longitude,
  countIf(time >= toDateTime('{sql_time(start24)}')) AS spots_24h,
  countIf(time >= toDateTime('{sql_time(start72)}')) AS spots_72h,
  count() AS spots_168h,
  uniqExactIf(toDate(time), time >= toDateTime('{sql_time(start24)}')) AS active_days_24h,
  uniqExactIf(toDate(time), time >= toDateTime('{sql_time(start72)}')) AS active_days_72h,
  uniqExact(toDate(time)) AS active_days_168h,
  min(time) AS first_seen_168h,
  max(time) AS last_seen_168h
FROM wspr.rx
WHERE time >= toDateTime('{sql_time(start168)}')
  AND time < toDateTime('{sql_time(end)}')
  AND band IN ({bands})
  AND code = 1
  AND rx_sign != ''
  AND rx_loc != ''
  AND rx_lat BETWEEN -90 AND 90
  AND rx_lon BETWEEN -180 AND 180
GROUP BY band, reporter_call, reporter_grid
ORDER BY band, reporter_call, reporter_grid
FORMAT CSVWithNames"""


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as fh:
        return list(csv.DictReader(fh))


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def maidenhead_center(grid: str) -> tuple[float, float] | None:
    g = grid.strip().upper()
    if len(g) < 4 or not ("A" <= g[0] <= "R" and "A" <= g[1] <= "R") or not g[2:4].isdigit():
        return None
    lon = (ord(g[0]) - 65) * 20 - 180
    lat = (ord(g[1]) - 65) * 10 - 90
    lon += int(g[2]) * 2
    lat += int(g[3])
    lon_size, lat_size = 2.0, 1.0
    if len(g) >= 6 and "A" <= g[4] <= "X" and "A" <= g[5] <= "X":
        lon += (ord(g[4]) - 65) * (2 / 24)
        lat += (ord(g[5]) - 65) * (1 / 24)
        lon_size, lat_size = 2 / 24, 1 / 24
    return lat + lat_size / 2, lon + lon_size / 2


def clean_text(value: object) -> str:
    return re.sub(r"\s+", " ", re.sub(r"<[^>]*>", " ", str(value or ""))).strip()


def rbn_reduced(records: list[dict[str, object]]) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for item in records:
        call = clean_text(item.get("call")).upper()
        grid = clean_text(item.get("grid")).upper()
        point = maidenhead_center(grid)
        if not call or point is None:
            continue
        bands = []
        raw = item.get("band")
        values = raw.values() if isinstance(raw, dict) else raw if isinstance(raw, list) else []
        for value in values:
            if isinstance(value, list) and len(value) > 1:
                band = clean_text(value[1])
                if band and band not in bands:
                    bands.append(band)
        rows.append({
            "skimmer_call": call,
            "skimmer_grid": grid,
            "latitude": f"{point[0]:.5f}",
            "longitude": f"{point[1]:.5f}",
            "continent": clean_text(item.get("cont")),
            "advertised_bands": " ".join(sorted(bands)),
            "last_seen": clean_text(item.get("lst_age")),
            "spot_policy": clean_text(item.get("sk_opt")),
            "skimmer_version": clean_text(item.get("sk_ver")),
        })
    return rows


def write_dict_csv(path: Path, rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not rows:
        raise RuntimeError(f"refusing to write empty CSV: {path}")
    with path.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(fh, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def refresh(snapshot_dir: Path, end: dt.datetime, cooldown: float) -> None:
    snapshot_dir.mkdir(parents=True, exist_ok=True)
    starts = {hours: end - dt.timedelta(hours=hours) for hours in WINDOWS}
    queries: dict[str, str] = {}
    for index, hours in enumerate(WINDOWS):
        query = receiver_query(starts[hours], end)
        queries[str(hours)] = query
        text = wspr_query(query)
        if not text.startswith("reporter_call,"):
            raise RuntimeError(f"unexpected WSPR.live response for {hours}h query")
        write_text(snapshot_dir / f"wspr-receivers-{hours}h.csv", text)
        if index < len(WINDOWS) - 1:
            time.sleep(cooldown)
    time.sleep(cooldown)
    bquery = band_query(starts[168], starts[72], starts[24], end)
    band_text = wspr_query(bquery)
    if not band_text.startswith("band,"):
        raise RuntimeError("unexpected WSPR.live response for band query")
    write_text(snapshot_dir / "wspr-receivers-by-band.csv", band_text)

    rbn_bytes = http_get(RBN_ENDPOINT)
    records = json.loads(rbn_bytes)
    if not isinstance(records, list) or not records:
        raise RuntimeError("unexpected RBN active-node response")
    write_text(snapshot_dir / "rbn-active-nodes.json", json.dumps(records, indent=2, sort_keys=True) + "\n")
    write_dict_csv(snapshot_dir / "rbn-active-nodes-reduced.csv", rbn_reduced(records))
    summary = summarize(snapshot_dir, end, queries, bquery)
    write_text(snapshot_dir / "summary.json", json.dumps(summary, indent=2) + "\n")


def count_window(rows: list[dict[str, str]], window_days: int | None = None) -> dict[str, object]:
    calls = {r["reporter_call"].upper() for r in rows if r.get("reporter_call")}
    pairs = {(r["reporter_call"].upper(), r["reporter_grid"].upper()) for r in rows if r.get("reporter_call") and r.get("reporter_grid")}
    grids = {r["reporter_grid"].upper()[:4] for r in rows if len(r.get("reporter_grid", "")) >= 4}
    total_spots = sum(int(r.get("spots", "0") or 0) for r in rows)
    top = Counter(r["reporter_grid"].upper()[:4] for r in rows if len(r.get("reporter_grid", "")) >= 4).most_common(25)
    return {
        "unique_reporter_grid_pairs": len(pairs),
        "unique_reporter_calls": len(calls),
        "unique_four_character_grids": len(grids),
        "total_spots": total_spots,
        "top_four_character_grids": top,
    }


def summarize(snapshot_dir: Path, end: dt.datetime, queries: dict[str, str] | None = None, bquery: str | None = None) -> dict[str, object]:
    windows: dict[str, object] = {}
    for hours in WINDOWS:
        rows = read_csv(snapshot_dir / f"wspr-receivers-{hours}h.csv")
        item = count_window(rows)
        item.update({"start_utc": (end - dt.timedelta(hours=hours)).isoformat(), "end_utc": end.isoformat()})
        if queries:
            item["query"] = queries[str(hours)]
        windows[str(hours)] = item

    band_rows = read_csv(snapshot_dir / "wspr-receivers-by-band.csv")
    band_summary: dict[str, object] = {}
    for band_value, band_name in POPULAR_BANDS.items():
        selected = [r for r in band_rows if int(r["band"]) == band_value]
        per_window: dict[str, object] = {}
        for hours in WINDOWS:
            spot_key = f"spots_{hours}h"
            active = [r for r in selected if int(r.get(spot_key, "0") or 0) > 0]
            per_window[str(hours)] = {
                "unique_reporter_grid_pairs": len({(r["reporter_call"].upper(), r["reporter_grid"].upper()) for r in active}),
                "unique_reporter_calls": len({r["reporter_call"].upper() for r in active}),
                "unique_four_character_grids": len({r["reporter_grid"].upper()[:4] for r in active if len(r["reporter_grid"]) >= 4}),
                "total_spots": sum(int(r.get(spot_key, "0") or 0) for r in active),
            }
        band_summary[band_name] = {"band_value": band_value, "windows": per_window}

    rbn_rows = read_csv(snapshot_dir / "rbn-active-nodes-reduced.csv")
    rbn_counts: dict[str, int] = {}
    rbn_grids: dict[str, int] = {}
    for band_name in POPULAR_BANDS.values():
        active = [r for r in rbn_rows if band_name in r.get("advertised_bands", "").split()]
        rbn_counts[band_name] = len(active)
        rbn_grids[band_name] = len({r["skimmer_grid"].upper()[:4] for r in active if len(r.get("skimmer_grid", "")) >= 4})
    return {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "end_utc": end.isoformat(),
        "mode": "WSPR-2 (code=1)",
        "all_hf_bands": list(ALL_HF_BANDS),
        "popular_bands": {str(k): v for k, v in POPULAR_BANDS.items()},
        "windows": windows,
        "wspr_popular_bands": band_summary,
        "wspr_popular_band_query": bquery or "See tools/why-not-rbn/snapshots/query-history.sql",
        "rbn_active_nodes": {
            "endpoint": RBN_ENDPOINT,
            "records_returned": len(rbn_rows),
            "usable_node_grid_pairs": len(rbn_rows),
            "band_counts": rbn_counts,
            "band_four_character_grids": rbn_grids,
        },
    }


def normalize_summary(snapshot_dir: Path) -> dict[str, object]:
    path = snapshot_dir / "summary.json"
    existing = json.loads(path.read_text(encoding="utf-8"))
    # Preserve the exact collected summary when available. Add derived RBN grid counts if absent.
    if "band_four_character_grids" not in existing.get("rbn_active_nodes", {}):
        rows = read_csv(snapshot_dir / "rbn-active-nodes-reduced.csv")
        existing["rbn_active_nodes"]["band_four_character_grids"] = {
            band: len({r["skimmer_grid"].upper()[:4] for r in rows if band in r.get("advertised_bands", "").split() and len(r.get("skimmer_grid", "")) >= 4})
            for band in POPULAR_BANDS.values()
        }
        write_text(path, json.dumps(existing, indent=2) + "\n")
    return existing


def world_symbol(world: dict[str, object]) -> str:
    paths: list[str] = []
    for feature in world.get("features", []):
        geometry = feature.get("geometry") or {}
        coords = geometry.get("coordinates") or []
        polygons = coords if geometry.get("type") == "MultiPolygon" else [coords] if geometry.get("type") == "Polygon" else []
        d_parts: list[str] = []
        for polygon in polygons:
            for ring in polygon:
                previous_x: float | None = None
                started = False
                for point in ring:
                    lon, lat = float(point[0]), float(point[1])
                    x, y = lon + 180.0, 90.0 - lat
                    if previous_x is not None and abs(x - previous_x) > 180:
                        started = False
                    d_parts.append(("M" if not started else "L") + f"{x:.2f},{y:.2f}")
                    started = True
                    previous_x = x
                if started:
                    d_parts.append("Z")
        if d_parts:
            paths.append(f'<path d="{" ".join(d_parts)}"/>')
    return '<symbol id="world" viewBox="0 0 360 180">' + ''.join(paths) + '</symbol>'


def point_xy(lat: float, lon: float, x: float, y: float, w: float, h: float) -> tuple[float, float]:
    return x + (lon + 180) / 360 * w, y + (90 - lat) / 180 * h


def load_points(snapshot_dir: Path) -> tuple[dict[str, dict[str, list[tuple[float, float]]]], dict[str, list[tuple[float, float]]]]:
    rows = read_csv(snapshot_dir / "wspr-receivers-by-band.csv")
    wspr: dict[str, dict[str, list[tuple[float, float]]]] = {band: {str(h): [] for h in WINDOWS} for band in POPULAR_BANDS.values()}
    value_to_name = POPULAR_BANDS
    for row in rows:
        name = value_to_name.get(int(row["band"]))
        if not name:
            continue
        lat, lon = float(row["latitude"]), float(row["longitude"])
        for hours in WINDOWS:
            if int(row.get(f"spots_{hours}h", "0") or 0) > 0:
                wspr[name][str(hours)].append((lat, lon))
    rbn_rows = read_csv(snapshot_dir / "rbn-active-nodes-reduced.csv")
    rbn: dict[str, list[tuple[float, float]]] = {band: [] for band in POPULAR_BANDS.values()}
    for row in rbn_rows:
        lat, lon = float(row["latitude"]), float(row["longitude"])
        bands = row.get("advertised_bands", "").split()
        for band in rbn:
            if band in bands:
                rbn[band].append((lat, lon))
    return wspr, rbn


def svg_start(width: int, height: int, title: str, symbol: str) -> list[str]:
    return [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">',
        f'<title id="title">{html.escape(title)}</title>',
        '<desc id="desc">Geographic and numerical comparison of WSPR reporting receivers and Reverse Beacon Network skimmers.</desc>',
        '<style>text{font-family:Inter,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;fill:#172026}.small{font-size:13px}.label{font-size:15px;font-weight:650}.heading{font-size:25px;font-weight:750}.subheading{font-size:17px;font-weight:650}.note{font-size:12px;fill:#52616b}.land{fill:#f1eee7;stroke:#aeb8bd;stroke-width:.55}.panel{fill:#fbfaf7;stroke:#cbd3d6;stroke-width:1}.grid{stroke:#e0e4e5;stroke-width:.6}.wspr{fill:#2d6cdf;fill-opacity:.34}.rbn{fill:#d45d00;fill-opacity:.82;stroke:#7f3700;stroke-width:.35}</style>',
        '<defs>', symbol, '</defs>',
        '<rect width="100%" height="100%" fill="#ffffff"/>',
    ]


def map_background(parts: list[str], x: float, y: float, w: float, h: float) -> None:
    parts.append(f'<rect class="panel" x="{x}" y="{y}" width="{w}" height="{h}" rx="5"/>')
    for lon in (-120, -60, 0, 60, 120):
        gx = x + (lon + 180) / 360 * w
        parts.append(f'<line class="grid" x1="{gx:.1f}" y1="{y}" x2="{gx:.1f}" y2="{y+h}"/>')
    for lat in (-60, -30, 0, 30, 60):
        gy = y + (90 - lat) / 180 * h
        parts.append(f'<line class="grid" x1="{x}" y1="{gy:.1f}" x2="{x+w}" y2="{gy:.1f}"/>')
    parts.append(f'<use href="#world" class="land" x="{x}" y="{y}" width="{w}" height="{h}"/>')


def create_maps(repo_root: Path, snapshot_dir: Path, world_path: Path, summary: dict[str, object], no_png: bool) -> None:
    assets = repo_root / "docs" / "assets" / "why-not-rbn"
    assets.mkdir(parents=True, exist_ok=True)
    world = json.loads(world_path.read_text(encoding="utf-8"))
    symbol = world_symbol(world)
    wspr, rbn = load_points(snapshot_dir)
    bands = ["40m", "20m", "15m"]

    # Six-panel footprint comparison.
    width, height = 1440, 850
    parts = svg_start(width, height, "WSPR and RBN receiver footprints by band", symbol)
    parts.append('<text class="heading" x="42" y="42">Receiver footprint by band</text>')
    parts.append('<text class="note" x="42" y="65">WSPR: distinct reporter/locator pairs seen during the seven-day window. RBN: active nodes advertising the band.</text>')
    margin_x, top, gap_x, gap_y = 42, 100, 22, 55
    panel_w = (width - 2 * margin_x - 2 * gap_x) / 3
    panel_h = 275
    for col, band in enumerate(bands):
        x = margin_x + col * (panel_w + gap_x)
        for row, network in enumerate(("WSPR", "RBN")):
            y = top + row * (panel_h + gap_y)
            points = wspr[band]["168"] if network == "WSPR" else rbn[band]
            map_background(parts, x, y, panel_w, panel_h)
            cls = "wspr" if network == "WSPR" else "rbn"
            radius = 1.55 if network == "WSPR" else 2.65
            for lat, lon in points:
                px, py = point_xy(lat, lon, x, y, panel_w, panel_h)
                parts.append(f'<circle class="{cls}" cx="{px:.2f}" cy="{py:.2f}" r="{radius}"/>')
            count = summary["wspr_popular_bands"][band]["windows"]["168"]["unique_reporter_grid_pairs"] if network == "WSPR" else summary["rbn_active_nodes"]["band_counts"][band]
            parts.append(f'<text class="subheading" x="{x+10:.1f}" y="{y-12:.1f}">{band} · {network} · {count:,}</text>')
    parts.append('<circle class="wspr" cx="45" cy="806" r="5"/><text class="small" x="57" y="811">WSPR reporter/locator pair</text>')
    parts.append('<circle class="rbn" cx="282" cy="806" r="5"/><text class="small" x="294" y="811">RBN active node</text>')
    parts.append('<text class="note" x="1398" y="811" text-anchor="end">Locator centers; not exact station addresses. Snapshot ends 2026-07-17 17:00 UTC.</text>')
    parts.append('</svg>')
    write_text(assets / "receiver-footprint-by-band.svg", ''.join(parts))

    # Individual overlay maps.
    for band in bands:
        width, height = 1200, 630
        parts = svg_start(width, height, f"{band} WSPR and RBN receiver footprint", symbol)
        parts.append(f'<text class="heading" x="40" y="42">{band} receiver footprint</text>')
        wcalls = summary["wspr_popular_bands"][band]["windows"]["168"]["unique_reporter_calls"]
        wpairs = summary["wspr_popular_bands"][band]["windows"]["168"]["unique_reporter_grid_pairs"]
        rn = summary["rbn_active_nodes"]["band_counts"][band]
        parts.append(f'<text class="note" x="40" y="65">Seven-day WSPR: {wcalls:,} calls / {wpairs:,} call-grid pairs. Current RBN: {rn:,} nodes advertising {band}.</text>')
        x, y, w, h = 40, 90, 1120, 500
        map_background(parts, x, y, w, h)
        for lat, lon in wspr[band]["168"]:
            px, py = point_xy(lat, lon, x, y, w, h)
            parts.append(f'<circle class="wspr" cx="{px:.2f}" cy="{py:.2f}" r="1.75"/>')
        for lat, lon in rbn[band]:
            px, py = point_xy(lat, lon, x, y, w, h)
            parts.append(f'<circle class="rbn" cx="{px:.2f}" cy="{py:.2f}" r="3.1"/>')
        parts.append('<circle class="wspr" cx="48" cy="615" r="5"/><text class="small" x="60" y="620">WSPR</text>')
        parts.append('<circle class="rbn" cx="128" cy="615" r="5"/><text class="small" x="140" y="620">RBN</text>')
        parts.append('</svg>')
        write_text(assets / f"receiver-network-{band}.svg", ''.join(parts))

    create_count_chart(assets, summary)
    create_grid_chart(assets, summary)
    create_explorer(assets, world, wspr, rbn, summary)

    if not no_png:
        try:
            import cairosvg  # type: ignore
            for svg in assets.glob("*.svg"):
                cairosvg.svg2png(url=str(svg), write_to=str(svg.with_suffix(".png")), output_width=1600)
        except Exception as exc:  # pragma: no cover - optional dependency
            print(f"PNG rendering skipped: {exc}", file=sys.stderr)


def create_count_chart(assets: Path, summary: dict[str, object]) -> None:
    width, height = 1100, 650
    parts = svg_start(width, height, "Receiver counts by band and time window", "")
    parts.append('<text class="heading" x="50" y="45">Distinct reporting calls by band</text>')
    parts.append('<text class="note" x="50" y="68">WSPR calls observed in bounded windows versus active RBN nodes advertising each band.</text>')
    bands = ["40m", "20m", "15m"]
    series = [("WSPR 24 h", "24", "#8fb5ff"), ("WSPR 72 h", "72", "#4e83e6"), ("WSPR 7 d", "168", "#174fae"), ("RBN snapshot", "rbn", "#d45d00")]
    max_value = max(summary["wspr_popular_bands"][b]["windows"]["168"]["unique_reporter_calls"] for b in bands) * 1.12
    left, top, chart_w, chart_h = 80, 105, 950, 440
    for tick in range(0, 1801, 300):
        y = top + chart_h - tick / max_value * chart_h
        parts.append(f'<line class="grid" x1="{left}" y1="{y:.1f}" x2="{left+chart_w}" y2="{y:.1f}"/>')
        parts.append(f'<text class="note" x="{left-12}" y="{y+4:.1f}" text-anchor="end">{tick:,}</text>')
    group_w = chart_w / len(bands)
    bar_w = 48
    gap = 12
    for bi, band in enumerate(bands):
        center = left + group_w * (bi + 0.5)
        total_w = len(series) * bar_w + (len(series)-1) * gap
        x0 = center - total_w / 2
        for si, (label, key, color) in enumerate(series):
            value = summary["rbn_active_nodes"]["band_counts"][band] if key == "rbn" else summary["wspr_popular_bands"][band]["windows"][key]["unique_reporter_calls"]
            h = value / max_value * chart_h
            x = x0 + si * (bar_w + gap)
            y = top + chart_h - h
            parts.append(f'<rect x="{x:.1f}" y="{y:.1f}" width="{bar_w}" height="{h:.1f}" rx="3" fill="{color}"/>')
            parts.append(f'<text class="small" x="{x+bar_w/2:.1f}" y="{y-7:.1f}" text-anchor="middle">{value:,}</text>')
        parts.append(f'<text class="label" x="{center:.1f}" y="{top+chart_h+31}" text-anchor="middle">{band}</text>')
    lx = 90
    for label, _, color in series:
        parts.append(f'<rect x="{lx}" y="595" width="18" height="18" rx="2" fill="{color}"/><text class="small" x="{lx+25}" y="609">{label}</text>')
        lx += 210
    parts.append('</svg>')
    write_text(assets / "receiver-counts-by-band.svg", ''.join(parts))


def create_grid_chart(assets: Path, summary: dict[str, object]) -> None:
    width, height = 980, 570
    parts = svg_start(width, height, "Distinct four-character Maidenhead grids by band", "")
    parts.append('<text class="heading" x="50" y="45">Geographic spread: occupied four-character grids</text>')
    parts.append('<text class="note" x="50" y="68">WSPR uses the seven-day window; RBN uses the active-node snapshot.</text>')
    bands = ["40m", "20m", "15m"]
    max_value = 620
    left, top, chart_w, chart_h = 80, 105, 830, 360
    for tick in range(0, 601, 100):
        y = top + chart_h - tick / max_value * chart_h
        parts.append(f'<line class="grid" x1="{left}" y1="{y:.1f}" x2="{left+chart_w}" y2="{y:.1f}"/>')
        parts.append(f'<text class="note" x="{left-12}" y="{y+4:.1f}" text-anchor="end">{tick}</text>')
    group_w = chart_w / 3
    for i, band in enumerate(bands):
        center = left + group_w * (i + .5)
        wv = summary["wspr_popular_bands"][band]["windows"]["168"]["unique_four_character_grids"]
        rv = summary["rbn_active_nodes"]["band_four_character_grids"][band]
        for offset, value, color, label in [(-42, wv, "#2d6cdf", "WSPR 7 d"), (42, rv, "#d45d00", "RBN")]:
            h = value / max_value * chart_h
            x = center + offset - 32
            y = top + chart_h - h
            parts.append(f'<rect x="{x:.1f}" y="{y:.1f}" width="64" height="{h:.1f}" rx="3" fill="{color}"/>')
            parts.append(f'<text class="small" x="{center+offset:.1f}" y="{y-7:.1f}" text-anchor="middle">{value}</text>')
        parts.append(f'<text class="label" x="{center:.1f}" y="{top+chart_h+30}" text-anchor="middle">{band}</text>')
    parts.append('<rect x="270" y="520" width="18" height="18" rx="2" fill="#2d6cdf"/><text class="small" x="295" y="534">WSPR 7 d</text>')
    parts.append('<rect x="500" y="520" width="18" height="18" rx="2" fill="#d45d00"/><text class="small" x="525" y="534">RBN active</text>')
    parts.append('</svg>')
    write_text(assets / "receiver-grid-footprint-by-band.svg", ''.join(parts))


def create_explorer(assets: Path, world: dict[str, object], wspr: dict[str, dict[str, list[tuple[float, float]]]], rbn: dict[str, list[tuple[float, float]]], summary: dict[str, object]) -> None:
    symbol = world_symbol(world).replace('<symbol id="world" viewBox="0 0 360 180">', '<g id="world" class="land">').replace('</symbol>', '</g>')
    data = {"wspr": wspr, "rbn": rbn, "summary": summary}
    page = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>WSPR and RBN receiver-band explorer</title>
<style>
:root{{font-family:Inter,system-ui,sans-serif;color:#172026;background:#f7f6f2}}body{{margin:0}}main{{max-width:1200px;margin:auto;padding:24px}}h1{{margin:.1em 0}}.controls{{display:flex;gap:20px;flex-wrap:wrap;padding:14px;background:white;border:1px solid #ccd4d7;border-radius:8px;margin:18px 0}}label{{font-weight:650}}select,input{{margin-left:7px}}#stats{{font-size:1.05rem;margin:10px 0 14px}}svg{{width:100%;height:auto;background:white;border:1px solid #ccd4d7;border-radius:8px}}.land{{fill:#f0ede5;stroke:#aeb8bd;stroke-width:.35}}.wspr{{fill:#2d6cdf;fill-opacity:.34}}.rbn{{fill:#d45d00;fill-opacity:.82;stroke:#7f3700;stroke-width:.2}}.note{{color:#52616b;font-size:.9rem;line-height:1.45}}
</style></head><body><main>
<h1>WSPR and RBN receiver footprint</h1>
<p class="note">WSPR points are distinct callsign/locator pairs observed in the selected window. RBN points are active nodes advertising the selected band. Locations are locator centers.</p>
<div class="controls"><label>Band <select id="band"><option>40m</option><option selected>20m</option><option>15m</option></select></label><label>WSPR window <select id="window"><option value="24">24 hours</option><option value="72">72 hours</option><option value="168" selected>7 days</option></select></label><label><input id="showW" type="checkbox" checked> WSPR</label><label><input id="showR" type="checkbox" checked> RBN</label></div>
<div id="stats"></div>
<svg id="map" viewBox="0 0 1000 500" role="img" aria-label="World receiver map"><g transform="translate(0 0) scale(2.77778)">{symbol}</g><g id="points"></g></svg>
<p class="note">This is a receiver-population map, not a calibrated sensitivity map. A missing point does not imply no propagation, and the two networks use different decoders and reporting rules.</p>
<script>const DATA={json.dumps(data, separators=(',', ':'))};
const band=document.querySelector('#band'),win=document.querySelector('#window'),showW=document.querySelector('#showW'),showR=document.querySelector('#showR'),points=document.querySelector('#points'),stats=document.querySelector('#stats');
function xy(p){{return [(p[1]+180)/360*1000,(90-p[0])/180*500]}}
function render(){{const b=band.value,w=win.value;let out='';if(showW.checked)for(const p of DATA.wspr[b][w]){{const q=xy(p);out+=`<circle class="wspr" cx="${{q[0].toFixed(2)}}" cy="${{q[1].toFixed(2)}}" r="1.7"/>`}}if(showR.checked)for(const p of DATA.rbn[b]){{const q=xy(p);out+=`<circle class="rbn" cx="${{q[0].toFixed(2)}}" cy="${{q[1].toFixed(2)}}" r="3.1"/>`}}points.innerHTML=out;const s=DATA.summary.wspr_popular_bands[b].windows[w],r=DATA.summary.rbn_active_nodes.band_counts[b];stats.innerHTML=`<strong>${{b}}</strong>: WSPR ${{w==='168'?'7-day':w+'-hour'}} — ${{s.unique_reporter_calls.toLocaleString()}} calls, ${{s.unique_reporter_grid_pairs.toLocaleString()}} callsign/grid pairs, ${{s.unique_four_character_grids.toLocaleString()}} four-character grids. RBN snapshot — ${{r.toLocaleString()}} active nodes.`}}
for(const e of [band,win,showW,showR])e.addEventListener('change',render);render();</script>
</main></body></html>"""
    write_text(assets / "receiver-network-band-explorer.html", page)


def snapshot_section(summary: dict[str, object]) -> str:
    end = str(summary["end_utc"]).replace("+00:00", "Z")
    start = str(summary["windows"]["168"]["start_utc"]).replace("+00:00", "Z")
    lines = [
        '<!-- BEGIN GENERATED RECEIVER SNAPSHOT -->',
        f'**Snapshot interval:** WSPR data from `{start}` through `{end}`; RBN active nodes fetched near the end of that interval.',
        '',
        '| Band | WSPR calls, 24 h | WSPR calls, 72 h | WSPR calls, 7 d | RBN active nodes | 7-day WSPR / RBN |',
        '| --- | ---: | ---: | ---: | ---: | ---: |',
    ]
    for band in ("40m", "20m", "15m"):
        w = summary["wspr_popular_bands"][band]["windows"]
        r = summary["rbn_active_nodes"]["band_counts"][band]
        ratio = w["168"]["unique_reporter_calls"] / r if r else math.nan
        lines.append(f'| {band} | {w["24"]["unique_reporter_calls"]:,} | {w["72"]["unique_reporter_calls"]:,} | {w["168"]["unique_reporter_calls"]:,} | {r:,} | {ratio:.1f}× |')
    lines.extend([
        '',
        'The all-HF WSPR queries found '
        f'{summary["windows"]["24"]["unique_reporter_calls"]:,} distinct reporter calls in 24 hours, '
        f'{summary["windows"]["72"]["unique_reporter_calls"]:,} in 72 hours, and '
        f'{summary["windows"]["168"]["unique_reporter_calls"]:,} in seven days. '
        f'The RBN endpoint returned {summary["rbn_active_nodes"]["records_returned"]:,} active nodes in its point-in-time snapshot.',
        '<!-- END GENERATED RECEIVER SNAPSHOT -->',
    ])
    return '\n'.join(lines)


def update_article(repo_root: Path, summary: dict[str, object]) -> None:
    path = repo_root / "docs" / "why-not-just-use-rbn.md"
    text = path.read_text(encoding="utf-8")
    replacement = snapshot_section(summary)
    pattern = re.compile(r'<!-- BEGIN GENERATED RECEIVER SNAPSHOT -->.*?<!-- END GENERATED RECEIVER SNAPSHOT -->', re.S)
    if not pattern.search(text):
        raise RuntimeError(f"generated snapshot markers missing from {path}")
    write_text(path, pattern.sub(replacement, text))


def write_query_history(snapshot_dir: Path, summary: dict[str, object]) -> None:
    lines = ["-- Exact WSPR.live queries used for this snapshot.\n"]
    for hours in WINDOWS:
        query = summary.get("windows", {}).get(str(hours), {}).get("query")
        if query:
            lines.append(f"-- {hours}-hour all-HF receiver query\n{query};\n\n")
    bquery = summary.get("wspr_popular_band_query")
    if bquery:
        lines.append(f"-- One bounded per-band query for 40, 20, and 15 meters\n{bquery};\n")
    write_text(snapshot_dir / "query-history.sql", ''.join(lines))


def main() -> int:
    args = parse_args()
    repo_root = args.repo_root.resolve()
    snapshot_dir = args.snapshot_dir.resolve()
    if args.refresh:
        refresh(snapshot_dir, utc_end(args.end), args.cooldown)
    summary = normalize_summary(snapshot_dir)
    write_query_history(snapshot_dir, summary)
    create_maps(repo_root, snapshot_dir, args.world_geojson.resolve(), summary, args.no_png)
    update_article(repo_root, summary)
    print(json.dumps({
        "mode": "refresh" if args.refresh else "offline",
        "snapshot_end": summary["end_utc"],
        "article": str(repo_root / "docs" / "why-not-just-use-rbn.md"),
        "assets": str(repo_root / "docs" / "assets" / "why-not-rbn"),
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
