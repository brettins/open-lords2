use super::*;

/// With *Advanced Farming* on, each saved industry forecast (`+0x2A8`) is what
/// `Industry_LabourEstimate` (`0x0044F318`) writes: `Pct(workers / divisor,
/// ramp)` with the ramp of its last search step, `workers = population` (B97),
/// capped by the resource limit. Ramp: `Industry_EfficiencyRamp` (`0x0044F248`)
/// written out from literals. Ablations: the flat 80 for the ramp, and the
/// ramp over workers for over population, each fail on advfarm-*.sav.
#[test]
fn advanced_farming_saves_forecast_by_the_efficiency_ramp() {
    // Call sites in `County_RefreshEstimates` 0x004485A5: (industry, job, base, divisor).
    const JOB: [u32; 4] = [6, 4, 7, 5];
    const BASE: [i32; 4] = [20, 15, 15, 15];
    const DIVISOR: [i32; 4] = [1, 1, 4, 2];
    const WEAPONS: u32 = 2;

    let mut checked = 0usize;
    let mut ramped = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        if s.globals().unwrap().opt_advanced_farming == 0 {
            continue;
        }
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            let band = s.u8_at(base + 0xB8).unwrap();
            let population = s.i32_at(base + 0x24).unwrap();
            for c in 0..4u32 {
                let rec = base + 0x294 + c * 0x18;
                let has = s.u8_at(rec + 0x01).unwrap() != 0;
                let countdown = s.u8_at(rec + 0x02).unwrap();
                let on = s.u8_at(rec + 0x03).unwrap() != 0;
                let last = i32::from(s.i8_at(rec + 0x08).unwrap()); // +0x29C
                let capacity = i32::from(s.i16_at(rec + 0x0A).unwrap()); // +0x29E
                let workers = s.i32_at(base + 0xC4 + JOB[c as usize] * 0x0C).unwrap();
                let b = BASE[c as usize];
                // 0x0044F248 with g_optAdvancedFarming != 0.
                let ramp = |n: i32| {
                    if n == 0 {
                        return 0;
                    }
                    let mut inc = b;
                    if capacity < n {
                        inc = b * (capacity * 100 / n) / 100;
                    }
                    (last + inc).min(100).max(b)
                };
                let own = if owner == 0 || band == 0 || !has || !on || countdown != 0 {
                    0
                } else {
                    ((workers / DIVISOR[c as usize]) * ramp(population) / 100).min(999)
                };
                let stored = s.i32_at(rec + 0x14).unwrap();
                if c == WEAPONS {
                    assert!(
                        (0..=own).contains(&stored),
                        "{}: county {id} weapons stores {stored}, the ramp allows {own}",
                        f.label()
                    );
                } else {
                    assert_eq!(stored, own, "{}: county {id} commodity {c}", f.label());
                }
                checked += 1;
                ramped += usize::from(own != 0 && own != (workers / DIVISOR[c as usize]) * 80 / 100);
            }
        }
    }
    if checked == 0 {
        eprintln!("no Advanced Farming save on this machine; nothing checked");
        return;
    }
    assert!(ramped > 0, "no forecast differs from the flat 80, so the ramp is untested");
    eprintln!("{checked} forecasts, {ramped} off the flat 80");
}
