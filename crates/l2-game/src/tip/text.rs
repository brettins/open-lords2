#![allow(unused_imports)]
use super::*;
use super::view::*;
use super::update_part::*;
use crate::message::{self, Record};
use crate::screen::ScreenId;
use crate::Game;

/// **Our transcription of groups 200…218**, for an install whose `L2.eng`
/// cannot be read. `CLAUDE.md` rule 6: the words are the feature, so the
/// player's own file is what is drawn and this is only the fallback.
pub const TEXT: &[(u16, &[&str])] = &[
    (
        200,
        &[
            "Game Objectives:",
            "Feed your peasants to keep them happy and make them multiply.",
            "Make weapons and create an army.",
            "Conquer thy neighbors!",
        ],
    ),
    (
        201,
        &[
            "Getting started:",
            "Right click on items for information.  Left click to perform actions.",
            "Use the slider bar to divide peasants between farming and industry (forestry, iron mining, stone quarrying and weapon making).",
            "Clicking on industries on the map switches them on and off.",
            "Changes you make in a county do not take place until the following season.",
        ],
    ),
    (
        202,
        &[
            "Food and Happiness:",
            "Cattle provide dairy foods each season, or they can be eaten.",
            "Wheat can be bought from a merchant and planted in winter. Click on a fallow field to change its usage.",
            "Plan ahead several seasons. Don't get caught without food for your peasants.",
            "High taxes and army recruiting lower happiness.",
            "Very low happiness could result in riots.",
        ],
    ),
    (206, &["Kingdom overview:", "Click on a county to go to that county"]),
    (
        207,
        &[
            "The Town Center:",
            "Click on any area for details about it.",
            "Click and drag a box around any peasants you wish to move, then click on the task area you wish to allocate them to.  Double-click town center to gather all idle workers.",
            "Blackened figures indicate there are not enough workers to meet optimal production in that task area.",
            "Idle peasants appear in an area if you allocate more than enough labor for the task.",
        ],
    ),
    (
        208,
        &[
            "The Blacksmith:",
            "To build a weapon, you must produce or buy the necessary amounts of iron and/or wood.",
        ],
    ),
    (
        209,
        &[
            "The Armoury:",
            "Use the slider bar to draft peasants from the population into the army.",
            "Don't draft too much of your population or happiness will drop too low.",
            "Choose units by clicking on the appropriate weapon on the wall. You can only raise the types of units you have weapons for.",
            "When your army is ready, click the Create button. Your army appears on the main map when you return to it.",
        ],
    ),
    (
        210,
        &[
            "Army Movement:",
            "After raising an army, move it toward a neighboring county.",
            "To move an army, click once on it, then click on a destination. The army marches toward the destination until it runs out of moves for the turn.",
            "Right clicking on an army calls up its vital statistics.",
        ],
    ),
    (
        211,
        &[
            "Invasions:",
            "To try to conquer a county, attack it by moving your army on top of the town center.",
        ],
    ),
    (
        212,
        &[
            "Battles:",
            "To move your units, click and drag a box around any number of them, then click on a destination.",
            "To attack, select units and then move the cursor onto an enemy unit. When the cursor turns red, click on the unit and your soldiers will attack it.",
        ],
    ),
    (
        214,
        &[
            "Sieges:",
            "Get your soldiers inside the castle to fight the defenders. To do this, break through the castle wall or gate with a catapult or battering ram, or go over the castle wall using a siege tower.",
            "Battering rams and siege towers must be moved right up to the castle wall to work. A catapult can attack from a distance.",
            "If the castle has a moat, select some of your soldiers (preferably peasants) and position the cursor over the water.  Click on the water and the soldiers will begin filling in the moat.",
        ],
    ),
    (215, &["Sieges2"]),
    (
        217,
        &[
            "Castle Building:",
            "Building a castle makes it much harder for enemy troops to conquer your county.",
            "Start with a simple castle design, then upgrade as your materials and builders increase.",
            "You may produce castle-building materials (stone and wood) in your county or buy them from a merchant.",
            "When you give orders to build a castle, some of your laborers in industry will move to castle building until the castle is finished.",
            "If your castle suffers damage from a siege, you may repair it as long as you have the materials and the builders.",
        ],
    ),
    (
        218,
        &[
            "Advanced Game Options:",
            "When Advanced Farming is on, the number of peasants required for grain farming varies from season to season, and weather and soil fertility affect wheat output.",
            "When Army Foraging is turned on, armies eat food in the county they're traveling in.",
            "When Exploration is turned on, the world outside your county is blacked out. It is gradually revealed as your armies move through and conquer new counties.",
        ],
    ),
];

pub fn transcribed(group: u16, index: usize) -> &'static str {
    TEXT.iter()
        .find(|(g, _)| *g == group)
        .and_then(|(_, s)| s.get(index))
        .copied()
        .unwrap_or("")
}

/// **`Eng_DrawString(group, index)` for a tip**: the player's own `L2.eng`,
/// and our transcription only where the file gave nothing.
pub fn words(shell: &crate::shell::ShellAssets, group: u16, index: usize) -> String {
    let s = shell.text(group as usize, index);
    if s.is_empty() {
        transcribed(group, index).to_string()
    } else {
        s.to_string()
    }
}

pub fn is_tip_window(record: &Record) -> bool {
    matches!(record.shape(), message::Shape::Paragraphs(_))
}

