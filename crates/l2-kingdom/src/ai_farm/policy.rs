use super::*;

/// `0x004A3C67`'s opening line: `if (grain < 100) county.+0x1F4 += 100`.
///
/// County `+0x1F4` is the **unowned county's own purse** — the thing
/// `Merchant_Trade` pays out of when the realm is 0.
/// county is quietly given a hundred crowns each pass so that the cascade below
/// it has something to spend. Only [`FarmStyle::NeutralArable`] does it, which
/// is odd company for the one style whose lord does not exist.
///
/// **`[V]`**,
/// *first* statement
/// follow it, so the hundred crowns are available to the cascade that spends
/// them in the same pass.
///
/// > This used to end *"this crate has no field for that purse — it is
/// > `Market`'s business — so this reports the top-up"*.
/// > [`County::purse`] has existed the whole time; the function reported and
/// > **[`lay_out`] did not apply it**, which is the shape `docs/agents.md` calls
/// > *a comment that defers work to a caller must name the caller*. `lay_out`
/// > applies it now.
pub const NEUTRAL_PURSE_TOP_UP: i32 = 100;

/// Whether this pass hands the county [`NEUTRAL_PURSE_TOP_UP`] before shopping.
pub fn neutral_purse_top_up(style: FarmStyle, county: &County) -> i32 {
    if style == FarmStyle::NeutralArable && county.grain < 100 {
        NEUTRAL_PURSE_TOP_UP
    } else {
        0
    }
}

fn stock(county: &County, good: Good) -> i32 {
    match good {
        Good::Grain => county.grain,
        Good::Cattle => county.herd,
    }
}

/// Run one style's cascade against a market. Returns how many lots moved.
pub fn run_buys(
    style: FarmStyle,
    id: usize,
    county: &mut County,
    map: &mut CampaignMap,
    market: &mut dyn Market,
) -> i32 {
    let mut bought = 0;
    for line in style.buys() {
        if stock(county, line.good) < line.floor
            && market.buy(id, county, map, line.lot, line.good)
        {
            bought += 1;
        }
    }
    bought
}

// ---------------------------------------------------------------------------
// FUN_004A4782 — the AI's ration setter
// ---------------------------------------------------------------------------

/// `FUN_0044E6A3` — **not** `Food_Available`,
/// point of `crate::ration::food_available`'s warning.
///
/// ```c
/// f = 0;
/// if (herd  > 0) f  = herd  * g_dairyPerHead;   /* 5  */
/// if (herd  > 0) f += herd  * g_foodPerHead;    /* 10 */
/// if (grain > 0) f += grain * g_foodPerSack;    /* 6  */
/// ```
///
/// Three functions in the binary compute a superficially similar sum from
/// different fields: this one from the **stores** (`herd`, `grain`),
/// `Food_Available` (`0x0044E7B4`) from the per-season *caps*
/// `herdAvailable`/`grainAvailable`, and `FUN_0044E741` from the stores with no
/// slaughter term at all. Picking the wrong one is `docs/decisions.md` C3
/// waiting to happen; the AI's ration ladder reads **this** one.
pub fn food_in_store(t: &Tables, county: &County) -> i32 {
    let mut food = 0i64;
    if county.herd > 0 {
        food += county.herd as i64 * t.food.dairy_per_head as i64;
        food += county.herd as i64 * t.food.food_per_head as i64;
    }
    if county.grain > 0 {
        food += county.grain as i64 * t.food.food_per_sack as i64;
    }
    food.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// How the style searches for a ration split — the `param_2` of `FUN_004A4782`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitSearch {
    /// `0` — walk 0 … 100 upward keeping the best, so ties go to the **lowest**
    /// split: prefer grain, spare the herd.
    PreferGrain,
    /// `100` — walk 100 … 0 downward keeping the best, so ties go to the
    /// **highest** split: eat the herd.
    PreferHerd,
    /// Anything else — take the number as the split, search nothing.
    Fixed(i32),
}

/// Which search a style asks for. Only the two **arable** styles ever ask for
/// [`SplitSearch::PreferHerd`], and only when the county already has a hundred
/// head to spare: `herd < 101 ? 0 : 100`.
pub fn split_search(style: FarmStyle, county: &County) -> SplitSearch {
    match style {
        FarmStyle::NeutralArable | FarmStyle::RealmArable => {
            if county.herd < 101 {
                SplitSearch::PreferGrain
            } else {
                SplitSearch::PreferHerd
            }
        }
        _ => SplitSearch::PreferGrain,
    }
}

/// The ration level the AI *asks* for, from what is in store —
/// `FUN_004A4782`'s first half.
///
/// ```c
/// store = food_in_store(county);  dairy = herd * 5;  people = pop (+ garrisons);
/// if      (store < people / 2) 0        /* None    */
/// else if (store < people)     1        /* Quarter */
/// else if (store < people * 2) 2        /* Half    */
/// else if (dairy > people * 3) 5        /* Triple  — on cheese alone */
/// else if (dairy > people * 2) 4        /* Double  */
/// else if (store > people * 8) 5
/// else if (store > people * 6) 4
/// else                         3        /* Normal  */
/// ```
///
/// **The two `dairy` rungs sit above the two large `store` rungs, and reading
/// them carefully turns the obvious story inside out.** `store` counts the herd
/// twice, at 5 and at 10, so `store >= 3 * dairy` always. Therefore:
///
/// * `dairy > 3p` implies `store > 9p > 8p`, so the **Triple rung is
///   redundant** — wherever it fires, the `store > 8p` rung below would have
///   said 5 anyway. It never changes an answer.
/// * `dairy > 2p` implies only `store > 6p`. When the store is also over `8p`
///   the earlier rung wins with **4** where the store rung would have given
///   **5**. So the surviving effect of the dairy rungs is a *downgrade*: a
///   county whose food is mostly cattle is fed **Double** where a county with
///   the same quantity of food as grain is fed **Triple**.
///
/// `[V]` on the arithmetic, `[D]` on calling it a bug. It is the only place in
/// the ration rules where the *composition* of the larder changes the level
///, and it costs the grazing lords a ration point.
pub fn ration_wanted(t: &Tables, county: &County, armies_eat: bool) -> i32 {
    let store = food_in_store(t, county);
    let dairy = ration::food_from_dairy(t, county.herd);
    let people = ration::people_to_feed(county, armies_eat);
    if store < people / 2 {
        0
    } else if store < people {
        1
    } else if store < people * 2 {
        2
    } else if dairy > people * 3 {
        5
    } else if dairy > people * 2 {
        4
    } else if store > people * 8 {
        5
    } else if store > people * 6 {
        4
    } else {
        3
    }
}

/// `FUN_004A4782` — set `rationWanted`, then find the `rationSplit` that feeds
/// the county best.
///
/// The search is a brute-force sweep of all 101 splits, scoring each by the
/// level `Ration_Apply` manages to achieve, and it keeps a strict improvement
/// only (`best < achieved`). That is what makes the direction matter: sweeping
/// upward keeps the **first** split that reaches the best level and sweeping
/// downward keeps the **last**, so the two modes are "the least herd that will
/// do" and "the most".
///
/// **A transcription note.** In the downward limb the original writes the
/// running best back into `rationSplit` *inside* the loop.
/// It makes no difference — the next iteration overwrites it,
/// iteration's write is the one that survives — so this is written the
/// straightforward way. `[D]`, and
/// the reason it is safe to tidy is that `Ration_Apply` reads `rationSplit`
/// only at the top of each iteration, after it has already been set to the
/// candidate.
///
/// The scan calls the crate's [`ration::choose`], which is pure, so nothing is
/// eaten while the AI is thinking. The original's `Ration_Apply` writes display
/// fields on every one of the 101 calls and this does not; the surviving state
/// is identical because the last call is repeated at the end either way.
pub fn set_rations(t: &Tables, county: &mut County, search: SplitSearch, armies_eat: bool) {
    county.ration_wanted = ration_wanted(t, county, armies_eat);
    let score = |county: &County, split: i32| -> i32 {
        let mut probe = county.clone();
        probe.ration_split = split;
        ration::choose(t, &probe, armies_eat).level
    };
    let chosen = match search {
        SplitSearch::Fixed(split) => split,
        SplitSearch::PreferGrain => {
            let mut best = 0;
            let mut chosen = 0;
            for split in 0..=100 {
                let achieved = score(county, split);
                if best < achieved {
                    best = achieved;
                    chosen = split;
                }
            }
            chosen
        }
        SplitSearch::PreferHerd => {
            let mut best = 0;
            let mut chosen = 100;
            for split in (0..=100).rev() {
                let achieved = score(county, split);
                if best < achieved {
                    best = achieved;
                    chosen = split;
                }
            }
            chosen
        }
    };
    county.ration_split = chosen;
    ration::preview(t, county, armies_eat);
}

// ---------------------------------------------------------------------------
// The field layout
// ---------------------------------------------------------------------------

/// The Winter grain quota, **including the unreachable rung**.
///
/// ```c
/// if      (fertility < -20) q = base - 1;
/// else if (fertility < -50) q = base - 2;   /* dead code */
/// else                      q = base;
/// ```
///
/// Finding 1 in the module documentation. `base` is `total / 2` for the two
/// arable styles and `total / 3` for the mixed one. The subtraction is done in
/// `unsigned` in the original
///
/// returned to fallow — which `i32` here reproduces exactly.
pub fn winter_grain_quota(fertility: i32, base: i32) -> i32 {
    if fertility < -20 {
        base - 1
    } else if fertility < -50 {
        base - 2
    } else {
        base
    }
}

/// `Labour_DefaultSharesBuilt` (`0x0045158B`) — the setter every farming style
/// uses. Farm 33/50/17, industry 40/15/15/15/15.
///
/// Written here because it is the AI's choice
/// of the two, and because the shares are two independent groups each summing
/// to 100 — an invariant [`crate::county::County::labour_share`] documents and
/// this upholds.
pub fn default_shares_built(county: &mut County) {
    use crate::tables::{
        JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_FIELD_RECLAMATION,
        JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
    };
    county.labour_share[JOB_GRAIN_FARMING] = 33;
    county.labour_share[JOB_CATTLE_FARMING] = 50;
    county.labour_share[JOB_FIELD_RECLAMATION] = 17;
    county.labour_share[JOB_CASTLE_BUILDING] = 40;
    county.labour_share[JOB_IRON_MINING] = 15;
    county.labour_share[JOB_STONE_QUARRYING] = 15;
    county.labour_share[JOB_WOOD_CUTTING] = 15;
    county.labour_share[JOB_BLACKSMITH] = 15;
}

/// `FUN_004A4694` — nudge the pasture count by one, and only by one.
///
/// ```c
/// if (herdCrowding < 11) {
///     if (fieldsCattle > 2 && herd < 10) set pasture count to fieldsCattle - 1;
/// } else if (fieldsCattle < cap) {
///     set pasture count to fieldsCattle + 1;
/// }
/// recount; update crowding;
/// ```
///
/// So the herd's own crowding meter is the controller: **under 11 the county
/// gives a field back**, but only if it has more than two and fewer than ten
/// animals; at 11 or over it takes one more, up to `cap`. Both limbs clear
/// every pasture first and re-lay the whole count, which matters because
/// `set_count` fills in **slot order** — the pasture the county keeps after a
/// shrink is not necessarily the one it had.
///
/// The original's `else if` re-tests `10 < herdCrowding`, which is already
/// implied. Redundant, not a second condition; dropped here.
pub fn fit_cattle_fields(t: &Tables, county: &mut County, map: &mut CampaignMap, cap: i32) {
    if county.herd_crowding < 11 {
        if county.fields_cattle > 2 && county.herd < 10 {
            field::clear_type(county, map, FieldType::Pasture);
            field::set_count(county, map, FieldType::Pasture, county.fields_cattle - 1);
        }
    } else if county.fields_cattle < cap {
        field::clear_type(county, map, FieldType::Pasture);
        field::set_count(county, map, FieldType::Pasture, county.fields_cattle + 1);
    }
    field::recount(county, map);
    field::herd_update_crowding(t, county, map);
}

