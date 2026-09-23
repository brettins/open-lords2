use super::*;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::conquest::ConquestScreen;
use l2_game::screens::setup::SetupPage;

/// Screen_FrameInput 0x0042FF10, 00420000.c:7529-7531: `g_campaignMap >= 8` returns
/// to screen 0 on either button's release; a press does nothing.
#[test]
fn a_release_after_the_last_map_returns_to_the_title() {
    let (mut game, assets) = bare();
    game.campaign.map = l2_game::victory::CAMPAIGN_LENGTH;
    let mut s = ConquestScreen::new();
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(s.handle(Event::Click { x: 0, y: 0 }, &mut ctx), Transition::Stay);
    let title = Transition::Replace(ScreenId::Setup(SetupPage::Title));
    assert_eq!(s.handle(Event::Release { x: 0, y: 0 }, &mut ctx), title);
    assert_eq!(s.handle(Event::RightClick { x: 0, y: 0 }, &mut ctx), title);
}
