use std::sync::LazyLock;

/// IUGG mean Earth radius used for report-only great-circle geometry.
pub const EARTH_MEAN_RADIUS_KM: f64 = 6_371.008_8;
/// Half the circumference of the modeled Earth.
pub const EARTH_ANTIPODE_DISTANCE_KM: f64 = std::f64::consts::PI * EARTH_MEAN_RADIUS_KM;
/// Hard binary-size budget approved for the embedded coastline.
pub const NATURAL_EARTH_COASTLINE_MAX_BYTES: usize = 60 * 1024;
/// Checked-in byte count compiled into the report crate.
pub const NATURAL_EARTH_COASTLINE_BYTES: usize = NATURAL_EARTH_COASTLINE_ASSET.len();
/// Coastline vertices closer to the antipode are omitted from polar projections.
pub const COASTLINE_MAX_DISTANCE_KM: f64 = 19_000.0;
/// Projected jumps larger than this fraction of the world radius start a new path.
pub const COASTLINE_MAX_SEGMENT_JUMP_FRACTION: f64 = 0.18;

const NATURAL_EARTH_COASTLINE_ASSET: &str =
    include_str!("../assets/natural-earth-110m-coastline.txt");
const _: () = assert!(NATURAL_EARTH_COASTLINE_BYTES <= NATURAL_EARTH_COASTLINE_MAX_BYTES);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeographicCoordinates {
    pub latitude_degrees: f64,
    pub longitude_degrees: f64,
}

impl GeographicCoordinates {
    pub fn new(latitude_degrees: f64, longitude_degrees: f64) -> Option<Self> {
        (latitude_degrees.is_finite()
            && longitude_degrees.is_finite()
            && (-90.0..=90.0).contains(&latitude_degrees)
            && (-180.0..=180.0).contains(&longitude_degrees))
        .then_some(Self {
            latitude_degrees,
            longitude_degrees,
        })
    }

    fn is_valid(self) -> bool {
        Self::new(self.latitude_degrees, self.longitude_degrees).is_some()
    }
}

/// Resolves the center of a normalized 4-, 6-, or 8-character Maidenhead cell.
///
/// ASCII case and surrounding whitespace are normalized locally. Invalid or
/// missing locators return `None`; no fallback coordinate is invented.
pub fn station_coordinates_from_grid(grid: &str) -> Option<GeographicCoordinates> {
    let grid = grid.trim().as_bytes();
    if !matches!(grid.len(), 4 | 6 | 8) {
        return None;
    }

    let field_lon = ascii_index(grid[0], b'A', b'R')?;
    let field_lat = ascii_index(grid[1], b'A', b'R')?;
    let square_lon = ascii_index(grid[2], b'0', b'9')?;
    let square_lat = ascii_index(grid[3], b'0', b'9')?;
    let mut longitude = -180.0 + f64::from(field_lon) * 20.0 + f64::from(square_lon) * 2.0;
    let mut latitude = -90.0 + f64::from(field_lat) * 10.0 + f64::from(square_lat);

    let (width, height) = match grid.len() {
        4 => (2.0, 1.0),
        6 => {
            longitude += f64::from(ascii_index(grid[4], b'A', b'X')?) * 5.0 / 60.0;
            latitude += f64::from(ascii_index(grid[5], b'A', b'X')?) * 2.5 / 60.0;
            (5.0 / 60.0, 2.5 / 60.0)
        }
        8 => {
            longitude += f64::from(ascii_index(grid[4], b'A', b'X')?) * 5.0 / 60.0;
            latitude += f64::from(ascii_index(grid[5], b'A', b'X')?) * 2.5 / 60.0;
            longitude += f64::from(ascii_index(grid[6], b'0', b'9')?) / 120.0;
            latitude += f64::from(ascii_index(grid[7], b'0', b'9')?) / 240.0;
            (1.0 / 120.0, 1.0 / 240.0)
        }
        _ => unreachable!(),
    };

    GeographicCoordinates::new(latitude + height / 2.0, longitude + width / 2.0)
}

fn ascii_index(value: u8, minimum: u8, maximum: u8) -> Option<u8> {
    let value = value.to_ascii_uppercase();
    (minimum..=maximum)
        .contains(&value)
        .then_some(value - minimum)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GreatCirclePosition {
    pub distance_km: f64,
    pub initial_bearing_degrees: f64,
}

/// Derives spherical great-circle distance and clockwise initial bearing.
pub fn great_circle_position(
    origin: GeographicCoordinates,
    destination: GeographicCoordinates,
) -> Option<GreatCirclePosition> {
    if !origin.is_valid() || !destination.is_valid() {
        return None;
    }

    let origin_latitude = origin.latitude_degrees.to_radians();
    let destination_latitude = destination.latitude_degrees.to_radians();
    let latitude_delta = destination_latitude - origin_latitude;
    let longitude_delta = (destination.longitude_degrees - origin.longitude_degrees).to_radians();
    let haversine = (latitude_delta / 2.0).sin().powi(2)
        + origin_latitude.cos()
            * destination_latitude.cos()
            * (longitude_delta / 2.0).sin().powi(2);
    let central_angle = 2.0 * haversine.sqrt().atan2((1.0 - haversine).max(0.0).sqrt());
    let y = longitude_delta.sin() * destination_latitude.cos();
    let x = origin_latitude.cos() * destination_latitude.sin()
        - origin_latitude.sin() * destination_latitude.cos() * longitude_delta.cos();

    Some(GreatCirclePosition {
        distance_km: EARTH_MEAN_RADIUS_KM * central_angle,
        initial_bearing_degrees: y.atan2(x).to_degrees().rem_euclid(360.0),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AzimuthalPoint {
    /// East-positive offset from the station center.
    pub x_km: f64,
    /// North-positive offset from the station center.
    pub y_km: f64,
    pub distance_km: f64,
    pub initial_bearing_degrees: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AzimuthalEquidistantProjection {
    center: GeographicCoordinates,
}

impl AzimuthalEquidistantProjection {
    pub fn new(center: GeographicCoordinates) -> Option<Self> {
        center.is_valid().then_some(Self { center })
    }

    pub fn center(self) -> GeographicCoordinates {
        self.center
    }

    /// Projects a coordinate with radial distance preserved in kilometers.
    pub fn project(self, coordinates: GeographicCoordinates) -> Option<AzimuthalPoint> {
        let position = great_circle_position(self.center, coordinates)?;
        let bearing = position.initial_bearing_degrees.to_radians();
        Some(AzimuthalPoint {
            x_km: position.distance_km * bearing.sin(),
            y_km: position.distance_km * bearing.cos(),
            distance_km: position.distance_km,
            initial_bearing_degrees: position.initial_bearing_degrees,
        })
    }

    /// Projects and safely splits the embedded Natural Earth coastline.
    pub fn project_coastline(self) -> Vec<ProjectedCoastlinePath> {
        project_coastline_paths(self, natural_earth_coastline())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarPoint {
    /// East-positive normalized offset in `[-1, 1]`.
    pub x: f64,
    /// North-positive normalized offset in `[-1, 1]`.
    pub y: f64,
    pub radius: f64,
    pub bearing_degrees: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarRing {
    pub distance_km: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquareRootPolarFrame {
    max_distance_km: f64,
}

impl SquareRootPolarFrame {
    pub fn new(max_distance_km: f64) -> Option<Self> {
        (max_distance_km.is_finite() && max_distance_km > 0.0).then_some(Self { max_distance_km })
    }

    pub fn max_distance_km(self) -> f64 {
        self.max_distance_km
    }

    pub fn radius(self, distance_km: f64) -> Option<f64> {
        (distance_km.is_finite() && (0.0..=self.max_distance_km).contains(&distance_km))
            .then_some((distance_km / self.max_distance_km).sqrt())
    }

    pub fn project(self, distance_km: f64, bearing_degrees: f64) -> Option<PolarPoint> {
        if !bearing_degrees.is_finite() {
            return None;
        }
        let radius = self.radius(distance_km)?;
        let bearing = bearing_degrees.rem_euclid(360.0).to_radians();
        Some(PolarPoint {
            x: radius * bearing.sin(),
            y: radius * bearing.cos(),
            radius,
            bearing_degrees: bearing_degrees.rem_euclid(360.0),
        })
    }

    pub fn rings(self, distances_km: &[f64]) -> Option<Vec<PolarRing>> {
        let mut previous = -1.0;
        distances_km
            .iter()
            .copied()
            .map(|distance_km| {
                if distance_km <= previous {
                    return None;
                }
                previous = distance_km;
                Some(PolarRing {
                    distance_km,
                    radius: self.radius(distance_km)?,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeographicPath {
    pub points: Vec<GeographicCoordinates>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedCoastlinePath {
    pub points: Vec<AzimuthalPoint>,
}

static NATURAL_EARTH_COASTLINE: LazyLock<Box<[GeographicPath]>> =
    LazyLock::new(parse_natural_earth_coastline);

pub fn natural_earth_coastline() -> &'static [GeographicPath] {
    &NATURAL_EARTH_COASTLINE
}

fn parse_natural_earth_coastline() -> Box<[GeographicPath]> {
    NATURAL_EARTH_COASTLINE_ASSET
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| GeographicPath {
            points: line
                .split(';')
                .map(|pair| {
                    let (longitude, latitude) = pair
                        .split_once(',')
                        .expect("embedded coastline point has two coordinates");
                    GeographicCoordinates::new(
                        latitude
                            .parse::<i16>()
                            .expect("embedded coastline latitude is an integer")
                            as f64
                            / 10.0,
                        longitude
                            .parse::<i16>()
                            .expect("embedded coastline longitude is an integer")
                            as f64
                            / 10.0,
                    )
                    .expect("embedded coastline point is in range")
                })
                .collect(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn project_coastline_paths(
    projection: AzimuthalEquidistantProjection,
    paths: &[GeographicPath],
) -> Vec<ProjectedCoastlinePath> {
    let max_jump_km = EARTH_ANTIPODE_DISTANCE_KM * COASTLINE_MAX_SEGMENT_JUMP_FRACTION;
    let mut projected_paths = Vec::new();

    for path in paths {
        let mut current = Vec::new();
        for coordinates in &path.points {
            let point = projection
                .project(*coordinates)
                .expect("embedded coastline coordinates are valid");
            let beyond_horizon = point.distance_km > COASTLINE_MAX_DISTANCE_KM;
            let jumps = current.last().is_some_and(|previous: &AzimuthalPoint| {
                (point.x_km - previous.x_km).hypot(point.y_km - previous.y_km) > max_jump_km
            });
            if beyond_horizon || jumps {
                push_projected_path(&mut projected_paths, &mut current);
            }
            if !beyond_horizon {
                current.push(point);
            }
        }
        push_projected_path(&mut projected_paths, &mut current);
    }

    projected_paths
}

fn push_projected_path(
    output: &mut Vec<ProjectedCoastlinePath>,
    current: &mut Vec<AzimuthalPoint>,
) {
    if current.len() >= 2 {
        output.push(ProjectedCoastlinePath {
            points: std::mem::take(current),
        });
    } else {
        current.clear();
    }
}

#[cfg(test)]
mod tests;
