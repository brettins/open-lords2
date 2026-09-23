#![allow(unused_imports)]
use super::*;
use crate::screens::setup::skirmish::ROWS_SHOWN;

/// **Page 12 answers two different hotspot tables**, and which one is tested
/// is `DAT_0056899C` — `Screen_HandleInput`'s `g_setupPage == 0xC` arm,
/// `004b0000.c:7471-7487`:
///
/// ```c
/// if (DAT_0056899C == 1) {
///   if (Widget_Test(0, 0, &DAT_004DE000, DAT_0056D5C0)) return 1;   the counts
///   if (Hotspot_Test(0, 0, &DAT_004DD130, 0x14))        return 1;   custom
/// } else {
///   if (Hotspot_Test(0, 0, &DAT_004DCF50, 0x14))        return 1;   normal
/// }
/// ```
///
/// Twenty records each, not the twenty-two an earlier reading of this file
/// took from `0x004DCF68` — that address is record **1**, so the dump began
/// one record in and wrapped round to three duplicates at the end. The two
/// tables share sixteen records and differ in four:
///
/// ```text
///          0x004DCF50 (normal)            0x004DD130 (custom)
///  11/12   FUN_0043DD83  (11,259,169,304) FUN_0043DBAC  (280,150,404,176)
///                        (278,259,436,304) FUN_0043DBAC (280,373,404,399)
///  18/19   FUN_0043DAF3  (20,350,180,475) FUN_0043DD83  (267,423,299,455)
///                        (275,350,435,475)  -- and nothing at 19
/// ```
///
/// So the handicap seesaw is **not** on the custom page: each realm's
/// difficulty caption cycles 0…4 on its own instead, and the sides swap from
/// one small box between the two stacks.
impl SetupScreen {
    pub(super) fn skirmish_arms(&self, v: &mut Vec<(Rect, Action)>) {
        // `FUN_0043D649` (*Back*), `FUN_0043DA9E` (*Cust.*) and
        // `FUN_0043D5B7` (*Go*), records 0…2 of both tables.
        for (i, x) in [461, 519, 577].iter().enumerate() {
            v.push((Rect::new(*x, 422, 56, 52), Action::Item(i)));
        }
        // Page 11's own arms — the lobby list — are **out of scope**,
        // `docs/plan.md` 0.1.
        if self.page == SetupPage::SkirmishMulti {
            return;
        }
        // `Widget_Test` runs before `Hotspot_Test`, so the count buttons take
        // a press the shared records would otherwise have.
        self.custom_count_arms(v);
        for r in 0..ROWS_SHOWN {
            let y = 185 + r as i32 * 16;
            v.push((Rect::new(468, y, 133, 15), Action::Skirmish(SkirmishArm::Row(r))));
        }
        for (y, d) in [(181, -1), (266, 1)] {
            v.push((Rect::new(604, y, 20, 20), Action::Skirmish(SkirmishArm::Scroll(d))));
        }
        for (kind, y) in [(2usize, 291), (0, 322), (1, 353), (3, 384)] {
            v.push((Rect::new(463, y, 30, 30), Action::Skirmish(SkirmishArm::Kind(kind))));
        }
        v.push((Rect::new(495, 384, 141, 30), Action::Skirmish(SkirmishArm::OpenFiles)));
        if self.skirmish.custom {
            for (i, y) in [150, 373].iter().enumerate() {
                v.push((
                    Rect::new(280, *y, 124, 26),
                    Action::Skirmish(SkirmishArm::Cycle(i + 1)),
                ));
            }
            v.push((Rect::new(267, 423, 32, 32), Action::Skirmish(SkirmishArm::Sides)));
            return;
        }
        for x in [11, 278] {
            v.push((Rect::new(x, 259, 158, 45), Action::Skirmish(SkirmishArm::Sides)));
        }
        for (i, x) in [20, 275].iter().enumerate() {
            v.push((
                Rect::new(*x, 350, 160, 125),
                Action::Skirmish(SkirmishArm::Handicap(i + 1)),
            ));
        }
    }
}

impl crate::screens::setup::skirmish::Skirmish {
    /// **`FUN_0043DBAC` (`0x0043DBAC`, `00430000.c:8153-8158`)** — the other
    /// difficulty arm: `realm[hotspot] + 0x24` steps on and wraps past 4, so
    /// this one reaches columns 0 and 4 that the seesaw cannot.
    pub fn cycle_difficulty(&mut self, realm: usize) {
        let Some(d) = self.difficulty.get_mut(realm.saturating_sub(1)) else { return };
        *d = if *d >= 4 { 0 } else { *d + 1 };
        self.edited = None;
    }
}
