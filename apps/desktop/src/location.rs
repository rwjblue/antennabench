use std::sync::Arc;

use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(
        dead_code,
        reason = "the complete typed result is produced by the macOS provider and consumed by the shared frontend"
    )
)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum LocationLookup {
    Success { latitude: f64, longitude: f64 },
    Denied,
    Restricted,
    Unavailable,
    Timeout,
}

trait LocationProvider: Send + Sync {
    fn request_one_shot(&self) -> LocationLookup;
}

pub(crate) struct LocationState(Arc<dyn LocationProvider>);

impl Default for LocationState {
    fn default() -> Self {
        Self(Arc::new(SystemLocationProvider))
    }
}

fn request_with_provider(provider: &dyn LocationProvider) -> LocationLookup {
    provider.request_one_shot()
}

#[tauri::command]
pub(crate) fn request_station_location(state: State<'_, LocationState>) -> LocationLookup {
    request_with_provider(state.0.as_ref())
}

struct SystemLocationProvider;

#[cfg(not(target_os = "macos"))]
impl LocationProvider for SystemLocationProvider {
    fn request_one_shot(&self) -> LocationLookup {
        LocationLookup::Unavailable
    }
}

#[cfg(target_os = "macos")]
impl LocationProvider for SystemLocationProvider {
    fn request_one_shot(&self) -> LocationLookup {
        macos::request_one_shot()
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        cell::{Cell, RefCell},
        time::{Duration, Instant},
    };

    use objc2::{
        define_class, rc::Retained, runtime::ProtocolObject, DefinedClass, MainThreadOnly,
    };
    use objc2_core_location::{
        kCLLocationAccuracyThreeKilometers, CLAuthorizationStatus, CLError, CLLocation,
        CLLocationManager, CLLocationManagerDelegate,
    };
    use objc2_foundation::{
        MainThreadMarker, NSArray, NSDate, NSError, NSObject, NSObjectProtocol, NSRunLoop,
    };

    use super::LocationLookup;

    const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);
    const RUN_LOOP_SLICE_SECONDS: f64 = 0.05;

    #[derive(Debug, Default)]
    struct LocationDelegateIvars {
        result: RefCell<Option<LocationLookup>>,
        location_requested: Cell<bool>,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements and this class is
        // confined to the main thread where Core Location delivers callbacks.
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[ivars = LocationDelegateIvars]
        struct LocationDelegate;

        // SAFETY: NSObjectProtocol adds no additional invariants.
        unsafe impl NSObjectProtocol for LocationDelegate {}

        // SAFETY: Each selector and argument type matches CLLocationManagerDelegate.
        unsafe impl CLLocationManagerDelegate for LocationDelegate {
            #[unsafe(method(locationManager:didUpdateLocations:))]
            fn did_update_locations(
                &self,
                _manager: &CLLocationManager,
                locations: &NSArray<CLLocation>,
            ) {
                let Some(location) = locations.lastObject() else {
                    self.finish(LocationLookup::Unavailable);
                    return;
                };
                // SAFETY: Core Location supplied a live CLLocation on the main thread.
                let coordinate = unsafe { location.coordinate() };
                // SAFETY: Core Location supplied the coordinate value.
                let outcome = if unsafe { coordinate.is_valid() }
                    && coordinate.latitude.is_finite()
                    && coordinate.longitude.is_finite()
                {
                    LocationLookup::Success {
                        latitude: coordinate.latitude,
                        longitude: coordinate.longitude,
                    }
                } else {
                    LocationLookup::Unavailable
                };
                self.finish(outcome);
            }

            #[unsafe(method(locationManager:didFailWithError:))]
            fn did_fail(&self, manager: &CLLocationManager, error: &NSError) {
                // A denial can arrive as either an authorization callback or a
                // Core Location error. Other provider failures remain unavailable.
                let outcome = if error.code() == CLError::Denied.0 {
                    authorization_outcome(unsafe { manager.authorizationStatus() })
                        .unwrap_or(LocationLookup::Denied)
                } else {
                    LocationLookup::Unavailable
                };
                self.finish(outcome);
            }

            #[unsafe(method(locationManagerDidChangeAuthorization:))]
            fn authorization_changed(&self, manager: &CLLocationManager) {
                self.continue_after_authorization(manager);
            }
        }
    );

    impl LocationDelegate {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(LocationDelegateIvars::default());
            // SAFETY: NSObject's initializer is valid for this subclass.
            unsafe { objc2::msg_send![super(this), init] }
        }

        fn finish(&self, outcome: LocationLookup) {
            let mut result = self.ivars().result.borrow_mut();
            if result.is_none() {
                *result = Some(outcome);
            }
        }

        fn continue_after_authorization(&self, manager: &CLLocationManager) {
            // SAFETY: The manager is alive and called on its delegate thread.
            let status = unsafe { manager.authorizationStatus() };
            if let Some(outcome) = authorization_outcome(status) {
                self.finish(outcome);
                return;
            }
            if is_authorized(status) && !self.ivars().location_requested.replace(true) {
                // SAFETY: The delegate is installed and requestLocation is one-shot.
                unsafe { manager.requestLocation() };
            }
        }

        fn take_result(&self) -> Option<LocationLookup> {
            self.ivars().result.borrow_mut().take()
        }
    }

    fn is_authorized(status: CLAuthorizationStatus) -> bool {
        status == CLAuthorizationStatus::AuthorizedAlways
            || status == CLAuthorizationStatus::AuthorizedWhenInUse
    }

    fn authorization_outcome(status: CLAuthorizationStatus) -> Option<LocationLookup> {
        if status == CLAuthorizationStatus::Denied {
            Some(LocationLookup::Denied)
        } else if status == CLAuthorizationStatus::Restricted {
            Some(LocationLookup::Restricted)
        } else {
            None
        }
    }

    pub(super) fn request_one_shot() -> LocationLookup {
        let Some(mtm) = MainThreadMarker::new() else {
            return LocationLookup::Unavailable;
        };
        // SAFETY: Core Location class methods are called on the main thread.
        if !unsafe { CLLocationManager::locationServicesEnabled_class() } {
            return LocationLookup::Unavailable;
        }

        // SAFETY: CLLocationManager is created and used only on the main thread.
        let manager = unsafe { CLLocationManager::new() };
        let delegate = LocationDelegate::new(mtm);
        // SAFETY: Both manager and delegate are live and main-thread confined.
        unsafe { manager.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
        // A coarse result is sufficient for a six-character Maidenhead grid.
        // SAFETY: The framework constant is valid for setDesiredAccuracy.
        unsafe { manager.setDesiredAccuracy(kCLLocationAccuracyThreeKilometers) };

        // SAFETY: The manager is alive and confined to the main thread.
        let initial_status = unsafe { manager.authorizationStatus() };
        if initial_status == CLAuthorizationStatus::NotDetermined {
            // This is the only place the macOS prompt is requested, and this
            // provider is invoked only by the explicit setup button command.
            // SAFETY: Info.plist contains NSLocationWhenInUseUsageDescription.
            unsafe { manager.requestWhenInUseAuthorization() };
        } else {
            delegate.continue_after_authorization(&manager);
        }

        let deadline = Instant::now() + LOOKUP_TIMEOUT;
        let run_loop = NSRunLoop::currentRunLoop();
        let outcome = loop {
            if let Some(outcome) = delegate.take_result() {
                break outcome;
            }
            if Instant::now() >= deadline {
                break LocationLookup::Timeout;
            }
            let slice = NSDate::dateWithTimeIntervalSinceNow(RUN_LOOP_SLICE_SECONDS);
            run_loop.runUntilDate(&slice);
        };
        // SAFETY: Clearing the weak delegate before releasing local objects is valid.
        unsafe { manager.setDelegate(None) };
        outcome
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use super::{request_with_provider, LocationLookup, LocationProvider};

    struct FakeProvider(Mutex<VecDeque<LocationLookup>>);

    impl FakeProvider {
        fn new(outcomes: impl IntoIterator<Item = LocationLookup>) -> Self {
            Self(Mutex::new(outcomes.into_iter().collect()))
        }
    }

    impl LocationProvider for FakeProvider {
        fn request_one_shot(&self) -> LocationLookup {
            self.0
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(LocationLookup::Unavailable)
        }
    }

    #[test]
    fn provider_outcomes_are_typed_and_repeated_requests_are_independent() {
        let provider = FakeProvider::new([
            LocationLookup::Denied,
            LocationLookup::Restricted,
            LocationLookup::Unavailable,
            LocationLookup::Timeout,
            LocationLookup::Success {
                latitude: 42.3601,
                longitude: -71.0589,
            },
        ]);
        assert_eq!(request_with_provider(&provider), LocationLookup::Denied);
        assert_eq!(request_with_provider(&provider), LocationLookup::Restricted);
        assert_eq!(
            request_with_provider(&provider),
            LocationLookup::Unavailable
        );
        assert_eq!(request_with_provider(&provider), LocationLookup::Timeout);
        assert_eq!(
            request_with_provider(&provider),
            LocationLookup::Success {
                latitude: 42.3601,
                longitude: -71.0589,
            }
        );
    }

    #[test]
    fn serialized_success_exposes_only_the_transient_coordinate_pair() {
        let value = serde_json::to_value(LocationLookup::Success {
            latitude: 42.3601,
            longitude: -71.0589,
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "status": "success",
                "latitude": 42.3601,
                "longitude": -71.0589,
            })
        );
    }
}
