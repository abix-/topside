//! Trading (design.md "Trading", authority.md "Trading"): the player asks
//! anyone to trade in conversation ("Let's trade."); how that person stands
//! toward them decides whether they will and at what prices (modforge
//! `trade::terms`); the trade panel shows both inventories side by side,
//! a click puts a stack on the table or takes it off, and Deal makes the
//! trade if they take it, from what each really carries (modforge
//! `trade::accepts`, `trade::exchange`). The one traded with stays where
//! they stand while it lasts, as in a conversation (talk::hold).

use bevy::prelude::*;
use modforge::actions::{Action, Said};
use modforge::hud::{Holder, OpenPanel, Trading};

use crate::actor::{Actor, Dead, Inventory, Memory, Npc};
use crate::content::{EpisodeRegistryRes, FactionRegistryRes, ItemRegistryRes, ItemValuesRes, PLAYER_FACTION};
use crate::input::ThisTick;
use crate::interact::Hud;
use crate::player::Player;

/// What the one asked says when they will not trade.
const REFUSES: &str = "I don't trade with you.";

/// Every step, the player's trading this tick: "Let's trade." said to
/// someone opens the trade panel with them if they will trade; a click on
/// a slot (`OfferSlot`) puts it on the table or takes it off; Deal makes
/// the trade if they take it and both have room. The panel closed, or the
/// one traded with gone or out of talking reach, the trade is over and
/// they are let go.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn trade(
    mut commands: Commands,
    items: Res<ItemRegistryRes>,
    values: Res<ItemValuesRes>,
    factions: Res<FactionRegistryRes>,
    episodes: Res<EpisodeRegistryRes>,
    mut players: Query<(Entity, &Actor, &Transform, &ThisTick, &mut Hud, &mut Inventory), With<Player>>,
    mut people: Query<(Entity, &Actor, &Name, &Transform, &mut Inventory, Option<&Npc>), (Without<Player>, Without<Dead>)>,
    memories: Query<(&Actor, &Memory, Option<&Npc>), Without<Dead>>,
    mut trading_with: Local<Option<Entity>>,
) {
    let Ok((player, me, here, this_tick, mut hud, mut mine)) = players.single_mut() else {
        return;
    };
    let heard = |words: &str| episodes.0.felt(words);
    // How someone stands toward the player, by the one rule, and so their
    // prices; None: they will not trade.
    let terms_of = |them: modforge::actor::ActorId, faction: Option<&str>| {
        let memory = memories.iter().find(|(a, ..)| a.0 == them).map(|(_, m, _)| m)?;
        let members = memories.iter().filter(|(_, _, n)| faction.is_some() && n.and_then(|n| n.faction.as_deref()) == faction).map(|(_, m, _)| m);
        modforge::trade::terms(crate::actor::stands_toward((memory, faction), members, (me.0, Some(PLAYER_FACTION)), &factions.0, heard))
    };
    for said in this_tick.0.iter().filter_map(|s| match s.action {
        Action::Talk { to, said: Said::Trade } => Some(to),
        _ => None,
    }) {
        let Some((them, actor, name, there, _, npc)) = people.iter().find(|(_, a, ..)| a.0.0 == said) else {
            continue;
        };
        match terms_of(actor.0, npc.and_then(|n| n.faction.as_deref())) {
            Some(_) => {
                hud.0.open_panel = OpenPanel::Trade;
                hud.0.trading = Some(Trading { with: said, name: name.to_string(), ..default() });
                // The conversation gives way to the trade; they stay put.
                hud.0.talking = None;
                commands.entity(player).remove::<crate::talk::Conversation>();
                crate::talk::hold(&mut commands, them, crate::interact::feet_of(there));
                *trading_with = Some(them);
            }
            None => {
                if let Some(talking) = hud.0.talking.as_mut() {
                    talking.line = Some((name.to_string(), REFUSES.to_string()));
                }
            }
        }
    }
    let with = hud.0.trading.as_ref().map(|t| t.with).filter(|_| hud.0.open_panel == OpenPanel::Trade);
    let partner = with.and_then(|w| people.iter_mut().find(|(_, a, ..)| a.0.0 == w));
    // Over: the panel closed, they are gone, or walked apart (a floor
    // between them too).
    let Some((_, actor, _, _, mut theirs, npc)) = partner.filter(|p| crate::talk::within(here.translation, p.3.translation, crate::talk::TALK_REACH * 2.0)) else {
        if hud.0.open_panel == OpenPanel::Trade {
            hud.0.open_panel = OpenPanel::None;
        }
        hud.0.trading = None;
        if let Some(them) = trading_with.take() {
            crate::talk::let_go(&mut commands, them);
        }
        return;
    };
    for s in &this_tick.0 {
        match s.action {
            Action::OfferSlot { slot } => {
                let count = match slot.holder {
                    Holder::Inventory => &mine.0,
                    Holder::Theirs => &theirs.0,
                    _ => continue,
                }
                .slots
                .get(slot.index)
                .and_then(|s| s.as_ref())
                .map_or(0, |s| s.count);
                if let Some(t) = hud.0.trading.as_mut() {
                    t.toggle(slot, count);
                }
            }
            Action::Deal => {
                let Some(t) = hud.0.trading.clone() else {
                    continue;
                };
                let terms = terms_of(actor.0, npc.and_then(|n| n.faction.as_deref()));
                let (Some(terms), Some(gets), Some(gives)) = (terms, modforge::trade::offered(&mine.0, &t.mine), modforge::trade::offered(&theirs.0, &t.theirs)) else {
                    continue;
                };
                let taken = modforge::trade::accepts(&values.0, terms, &gives, &gets);
                let made = taken && modforge::trade::exchange(&mut mine.0, &mut theirs.0, &t.mine, &t.theirs, |item| items.0.def(item).map_or(1, |d| d.max_stack)).is_ok();
                if let Some(t) = hud.0.trading.as_mut() {
                    if made {
                        t.mine.clear();
                        t.theirs.clear();
                    }
                    t.refused = !taken;
                    t.no_room = taken && !made;
                }
            }
            _ => {}
        }
    }
}
