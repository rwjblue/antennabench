use super::*;

const EPSILON: f64 = 1.0e-6;

fn coordinates(latitude_degrees: f64, longitude_degrees: f64) -> GeographicCoordinates {
    GeographicCoordinates::new(latitude_degrees, longitude_degrees).unwrap()
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn resolves_normalized_maidenhead_cell_centers_at_supported_precision() {
    assert_eq!(
        station_coordinates_from_grid(" FN31 "),
        Some(coordinates(41.5, -73.0))
    );

    let six = station_coordinates_from_grid("fn31pr").unwrap();
    assert_close(six.latitude_degrees, 41.729_166_666_7, EPSILON);
    assert_close(six.longitude_degrees, -72.708_333_333_3, EPSILON);

    let eight = station_coordinates_from_grid("FN31pr42").unwrap();
    assert_close(eight.latitude_degrees, 41.718_75, EPSILON);
    assert_close(eight.longitude_degrees, -72.712_5, EPSILON);
}

#[test]
fn rejects_missing_malformed_and_out_of_range_coordinates() {
    for invalid in [
        "",
        "FN3",
        "FN311",
        "SN31",
        "FN31ZQ",
        "FN31AA0X",
        "FN31AA000",
        "💥31",
    ] {
        assert_eq!(station_coordinates_from_grid(invalid), None, "{invalid:?}");
    }
    for (latitude, longitude) in [
        (f64::NAN, 0.0),
        (0.0, f64::INFINITY),
        (-90.1, 0.0),
        (90.1, 0.0),
        (0.0, -180.1),
        (0.0, 180.1),
    ] {
        assert_eq!(GeographicCoordinates::new(latitude, longitude), None);
    }
}

#[test]
fn derives_cardinal_and_intercardinal_great_circle_positions() {
    let origin = coordinates(0.0, 0.0);
    let north = great_circle_position(origin, coordinates(10.0, 0.0)).unwrap();
    let east = great_circle_position(origin, coordinates(0.0, 10.0)).unwrap();
    let northeast = great_circle_position(origin, coordinates(10.0, 10.0)).unwrap();

    assert_close(north.distance_km, 1_111.950_8, 0.001);
    assert_close(north.initial_bearing_degrees, 0.0, EPSILON);
    assert_close(east.distance_km, north.distance_km, EPSILON);
    assert_close(east.initial_bearing_degrees, 90.0, EPSILON);
    assert!((40.0..50.0).contains(&northeast.initial_bearing_degrees));
    assert!(northeast.distance_km > north.distance_km);
}

#[test]
fn azimuthal_projection_preserves_distance_and_bearing() {
    let projection = AzimuthalEquidistantProjection::new(coordinates(0.0, 0.0)).unwrap();
    assert_eq!(projection.center(), coordinates(0.0, 0.0));

    let north = projection.project(coordinates(10.0, 0.0)).unwrap();
    assert_close(north.x_km, 0.0, EPSILON);
    assert_close(north.y_km, north.distance_km, EPSILON);

    let east = projection.project(coordinates(0.0, 10.0)).unwrap();
    assert_close(east.x_km, east.distance_km, EPSILON);
    assert_close(east.y_km, 0.0, EPSILON);

    let northeast = projection.project(coordinates(10.0, 10.0)).unwrap();
    assert!(northeast.x_km > 0.0);
    assert!(northeast.y_km > 0.0);
    assert_close(
        northeast.x_km.hypot(northeast.y_km),
        northeast.distance_km,
        EPSILON,
    );
}

#[test]
fn square_root_polar_frame_pins_ring_and_direction_geometry() {
    let frame = SquareRootPolarFrame::new(16_000.0).unwrap();
    assert_eq!(frame.max_distance_km(), 16_000.0);
    assert_eq!(
        frame.rings(&[1_000.0, 4_000.0, 16_000.0]).unwrap(),
        vec![
            PolarRing {
                distance_km: 1_000.0,
                radius: 0.25,
            },
            PolarRing {
                distance_km: 4_000.0,
                radius: 0.5,
            },
            PolarRing {
                distance_km: 16_000.0,
                radius: 1.0,
            },
        ]
    );

    let north = frame.project(4_000.0, 0.0).unwrap();
    assert_close(north.x, 0.0, EPSILON);
    assert_close(north.y, 0.5, EPSILON);
    let east = frame.project(4_000.0, 450.0).unwrap();
    assert_close(east.x, 0.5, EPSILON);
    assert_close(east.y, 0.0, EPSILON);
    assert_eq!(east.bearing_degrees, 90.0);

    assert!(frame.rings(&[4_000.0, 1_000.0]).is_none());
    assert!(frame.project(16_001.0, 0.0).is_none());
    assert!(SquareRootPolarFrame::new(0.0).is_none());
}

#[test]
fn embedded_coastline_is_bounded_valid_and_deterministic() {
    assert_eq!(NATURAL_EARTH_COASTLINE_BYTES, 46_306);

    let coastline = natural_earth_coastline();
    assert_eq!(coastline.len(), 134);
    assert_eq!(
        coastline
            .iter()
            .map(|path| path.points.len())
            .sum::<usize>(),
        5_128
    );
    assert!(coastline.iter().all(|path| path.points.len() >= 2));

    let projection = AzimuthalEquidistantProjection::new(coordinates(41.5, -73.0)).unwrap();
    let first = projection.project_coastline();
    let second = projection.project_coastline();
    assert_eq!(first, second);
    assert!(!first.is_empty());
    assert!(first.iter().all(|path| path.points.len() >= 2));
    assert!(first.iter().flatten_points().all(|point| {
        point.distance_km <= COASTLINE_MAX_DISTANCE_KM
            && point.x_km.is_finite()
            && point.y_km.is_finite()
    }));

    let max_jump = EARTH_ANTIPODE_DISTANCE_KM * COASTLINE_MAX_SEGMENT_JUMP_FRACTION;
    assert!(first.iter().all(|path| path.points.windows(2).all(|pair| {
        (pair[1].x_km - pair[0].x_km).hypot(pair[1].y_km - pair[0].y_km) <= max_jump
    })));
}

#[test]
fn coastline_projection_drops_antipodes_and_splits_large_jumps() {
    let projection = AzimuthalEquidistantProjection::new(coordinates(0.0, 0.0)).unwrap();
    let paths = vec![GeographicPath {
        points: vec![
            coordinates(0.0, 0.0),
            coordinates(0.0, 1.0),
            coordinates(0.0, 180.0),
            coordinates(0.0, 2.0),
            coordinates(0.0, 3.0),
            coordinates(0.0, 150.0),
            coordinates(0.0, 151.0),
        ],
    }];
    let projected = project_coastline_paths(projection, &paths);

    assert_eq!(projected.len(), 3);
    assert!(projected.iter().all(|path| path.points.len() == 2));
    assert!(projected
        .iter()
        .flatten_points()
        .all(|point| point.distance_km <= COASTLINE_MAX_DISTANCE_KM));
}

trait FlattenPoints<'a> {
    fn flatten_points(self) -> Box<dyn Iterator<Item = &'a AzimuthalPoint> + 'a>;
}

impl<'a, I> FlattenPoints<'a> for I
where
    I: Iterator<Item = &'a ProjectedCoastlinePath> + 'a,
{
    fn flatten_points(self) -> Box<dyn Iterator<Item = &'a AzimuthalPoint> + 'a> {
        Box::new(self.flat_map(|path| path.points.iter()))
    }
}
