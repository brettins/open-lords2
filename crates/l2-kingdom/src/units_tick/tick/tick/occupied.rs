use super::*;
use crate::diplomacy::Letter;

/// What `Unit_EnterOccupiedTile` (`0x004658C1`) made of the tile in front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Arm {
    /// No occupant, or `Unit_Step`'s guards stop the unit before `Unit_TryEnterTile`.
    Free,
    /// Returns `local_8`: the mover steps onto the tile.
    Over,
    /// :4290 `Battle_BeginFromCampaign`; the path entry is given back (:4292).
    Battle(usize),
    /// :4267-4272 occupant human: walks over unless `+0x152` names it, which we do not
    /// model; our [`Contact::Blocked`] and the combine question stand in [I].
    Blocked(usize),
    /// :4301-4310 an enemy transport: `local_8 = 10`, the mover stays.
    Captured,
}

impl Kingdom {
    /// `Unit_EnterOccupiedTile` (`0x004658C1`), reached from `Unit_StepOnce` via
    /// `Unit_NeighbourTile`/`Unit_TryEnterTile` at a tile centre, for every kind `Unit_Step` walks.
    pub(super) fn enter_occupied(&mut self, id: usize, out: &mut UnitsTick) -> Arm {
        let units = &self.campaign.units;
        let Some(m) = units.get(id) else { return Arm::Free };
        // Unit_Step 00460000.c:4340/:4351 guards precede Unit_StepOnce [V].
        if m.path.is_empty() || m.moves_used >= m.move_allowance {
            return Arm::Free;
        }
        let (nx, ny) = m.path[0];
        let Some(occ) = units.at(nx, ny) else { return Arm::Free };
        let Some(o) = units.get(occ) else { return Arm::Free };
        // :4256-4264 merchants and transports move through anything; anything moves through a merchant.
        if matches!(m.kind, UnitKind::Merchant | UnitKind::Transport) || o.kind == UnitKind::Merchant {
            return Arm::Over;
        }
        let (mo, oo, okind, human) = (m.owner, o.owner, o.kind, o.owner_is_human);
        let join = crate::ai_army::Mission::JOIN_GARRISON;
        let errand = m.mission == join || o.mission == join;
        if matches!(okind, UnitKind::Army | UnitKind::PeasantMob) {
            if mo == oo {
                if human {
                    return Arm::Blocked(occ);
                }
                // :4274-4279 either side on mission 4 walks over without merging [V].
                if errand {
                    return Arm::Over;
                }
                // :4280 Army_Combine(g_movingUnit, occupant): the mover absorbs and walks on [V].
                match crate::unit::combine(&mut self.campaign.units, id, occ) {
                    Ok(_) => out.contacts.push(Contact::Merged { mover: id, into: id }),
                    // Army_Combine 004a0000.c:4356 `Msg_Enqueue(0, owner, 0xa7, ...)` [V].
                    Err(crate::unit::CombineRefusal::TwoMercenaryBands) => {
                        out.posted.push(Posted::Letter(Letter::system(mo, 167)));
                    }
                    // Army_Combine on two mobs merges them; ours refuses non-armies [I].
                    Err(_) => {}
                }
                return Arm::Over;
            }
            // :4284-4287 Diplo_ActionAllowed refuses: walk over [V].
            if !self.action_allowed(mo, oo) {
                return Arm::Over;
            }
            // :4289-4298 an auto-resolved battle (returns 0) walks on at once for owner != 0;
            // ours resolves after the sweep and steps on the next tick [I].
            return Arm::Battle(occ);
        }
        if okind == UnitKind::Transport && mo != oo && self.action_allowed(mo, oo) {
            // :4305-4309 Diplo_Offend(occupant, mover, 5); 0xa2 to the mover, 0xa1 to the occupant;
            // FUN_0046F0A9 frees the transport [V].
            for l in crate::diplomacy::offend(&mut self.realms, oo, mo, 5) {
                out.posted.push(Posted::Letter(l));
            }
            out.posted.push(Posted::Letter(Letter::system(mo, 162)));
            out.posted.push(Posted::Letter(Letter::system(oo, 161)));
            self.campaign.units.remove(occ);
            return Arm::Captured;
        }
        Arm::Over
    }

    /// Diplo_ActionAllowed 0x004A16F7; a mob's owner 6 has no realm record here, so only
    /// `actor != target` is read for it [I].
    fn action_allowed(&mut self, actor: u8, target: u8) -> bool {
        if actor as usize >= self.realms.len() {
            return target == 0 || actor != target;
        }
        crate::diplomacy::action_allowed(&mut self.realms, actor, target)
    }
}
