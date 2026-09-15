#![allow(unused_imports)]
use super::*;

use super::*;
use super::types::*;
use super::screen::*;
use super::painters::*;
use l2_kingdom::conquest::LeftCastle;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

/// **Our transcription of the words this arm draws**, used only where the
/// player's `L2.eng` has none — an install with no file, or a placeholder test
/// asset. Group 30's are the eighteen indices the farmland arm can reach.
pub(super) const TILE_WORDS: [(usize, &str); 49] = [
    (6, "Farmland"),
    (9, "Mine (iron)."),
    (10, "Quarry (stone)."),
    (11, "Lumber mill (timber)."),
    (12, "Flooded Field."),
    (13, "Parched Field."),
    (17, "- Barren."),
    (18, "- Lying Fallow."),
    (19, "- Wheat."),
    (20, "- Being reclaimed."),
    (21, "- Cattle."),
    (33, "Unusable at present. Farmers may be diverted from other tasks to reclaim this field."),
    (34, "Presently being rested. The more fallow land in a county, the higher its fertility for growing wheat."),
    (35, "This wheat field is currently unused."),
    (36, "This wheat field has just been sown."),
    (37, "This wheat field contains ripening crops."),
    (38, "This wheat field will be harvested next season."),
    (39, "This wheat field is ready for planting next season."),
    (40, "Over time and with continued labor, this field will return to a usable state and increase your overall farming capacity."),
    (41, "This field has improved somewhat, but needs further work before it is usable."),
    (42, "Partially restored, this field is on the way to returning to its former state."),
    (43, "Almost reclaimed, this field will very shortly be ready for use."),
    (52, "Click on an icon to alter field usage."),
    (53, "A small mine."),
    (54, "A medium mine."),
    (55, "A large mine."),
    (56, "A very large mine."),
    (57, "A destroyed mine."),
    (58, "Traditionally, fields were left fallow for a season as part of a crop rotation system."),
    (61, "A small quarry."),
    (62, "A medium quarry."),
    (63, "A large quarry."),
    (64, "A very large quarry."),
    (65, "A destroyed quarry."),
    (69, "A small lumber mill."),
    (70, "A medium lumber mill."),
    (71, "A large lumber mill."),
    (72, "A very large lumber mill."),
    (73, "A destroyed lumber mill."),
    (77, "This industry is shut down."),
    (78, "This industry is operational."),
    (80, "This field was flooded in the recent deluge, and any crops held within it were drowned."),
    (81, "This field was baked dry in the recent drought, and any crops held within it withered and died."),
    (82, "Blacksmith (armour)."),
    (83, "A small blacksmiths."),
    (84, "A medium blacksmiths."),
    (85, "A large blacksmiths."),
    (86, "A very large blacksmiths."),
    (87, "A destroyed blacksmiths."),
];

pub(super) const REPORT_WORDS: [&str; 29] = [
    "from",
    "to be sown, yielding",
    "in 4 seasons.",
    "harvested in",
    "sown in spring.",
    "Calf births expected",
    "Cow deaths expected",
    "Change due to farming",
    "Low herd crowding.",
    "Average herd crowding.",
    "Herd overcrowded.",
    "Massive overcrowding!!",
    "field being reclaimed",
    "fields being reclaimed",
    "Next field reclaimed in",
    "No field reclamation with 0 labourers",
    "gained last season, due to weather.",
    "lost last season, due to weather.",
    "Weather had no effect last season.",
    "No outside events affected the herd this season.",
    "died of disease.",
    "taken by wolves.",
    "had to be put down.",
    "born, over expectations.",
    "No outside factors affected stored grain.",
    "eaten by rats.",
    "found as surplus.",
    "Change due to eating",
    "Overall change",
];

pub(super) const FERTILITY_WORDS: [&str; 7] = [
    "Infertile - almost no production.",
    "Very poor fertility - mainly weeds.",
    "Poor fertility - crops grow less well.",
    "Average fertility - no effect on crops.",
    "Good fertility - crops are boosted.",
    "Very High fertility - many extra crops.",
    "Excellent fertility - bumper crop!",
];

