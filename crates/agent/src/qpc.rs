use retour::static_detour;

use std::mem::transmute;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::LibraryLoader::GetProcAddress;
use windows::core::{s, w};
use windows::core::BOOL;

type QueryPerformanceCounterFn = unsafe extern "system" fn(*mut i64) -> BOOL;

static_detour! {
    static QUERY_PERFORMANCE_COUNTER: unsafe extern "system" fn(*mut i64) -> BOOL;
}

pub unsafe fn setup_hook() {
    let target: QueryPerformanceCounterFn = unsafe {
        let hinst: HMODULE = GetModuleHandleW(w!("kernel32")).expect("Could not locate kernel32");
        let target =
            GetProcAddress(hinst, s!("QueryPerformanceCounter")).expect("Could not locate QueryPerformanceCounter");
        transmute(target)
    };

    unsafe {
        QUERY_PERFORMANCE_COUNTER
            .initialize(
                target,
                |perf_count: *mut i64| {
                    let ok = QUERY_PERFORMANCE_COUNTER.call(perf_count);

                    if ok == BOOL(0) {
                        return ok;
                    }

                    let scale = get_time_scale();
                    if (scale - 1.0).abs() < f64::EPSILON {
                        return ok;
                    }

                    // let original = *perf_count;
                    let scaled_f = (*perf_count as f64) * scale;
                    let scaled_i = if scaled_f.is_finite() {
                        scaled_f.clamp(i64::MIN as _, i64::MAX as _).round() as i64
                    } else {
                        *perf_count
                    };

                    *perf_count = scaled_i;

                    ok
                },
            )
            .unwrap()
            .enable()
            .unwrap();
    }
}

static TIME_SCALE_BITS: AtomicU64 = AtomicU64::new(f64::to_bits(1.0));

#[inline]
fn get_time_scale() -> f64 {
    f64::from_bits(TIME_SCALE_BITS.load(Ordering::Relaxed))
}

#[inline]
pub fn set_time_scale(scale: f64) {
    TIME_SCALE_BITS.store(f64::to_bits(scale), Ordering::Relaxed);
}
