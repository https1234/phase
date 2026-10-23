//! Casts the engine's own later steps reject must not be offered: an AI that
//! takes such an offer is rewound (CR 733) to the same decision and, picking
//! it again, never leaves it.
//!
//! - CR 601.2c + CR 107.3: a spell whose target depends on the announced X is
//!   castable only when some X the caster can announce leaves a legal target
//!   (March of Otherworldly Light with nothing to exile), and only those X
//!   values are offered.
//! - CR 700.2a + CR 601.2b-c: a mode whose target depends on X can't be chosen
//!   when no announceable X gives it one (Kozilek's Command with only {C}{C}).
//! - CR 601.2h: costs are paid in full or not at all, so a sacrifice for an
//!   additional cost is offered only when what is still owed stays payable
//!   (Deadly Precision can't sacrifice the only Treasure that makes its {B}).
//! - CR 601.2h: a cost that exiles an exact number of cards asks for exactly
//!   that many (Psychic Frog's three); a prompt allowing fewer filled the AI's
//!   capped candidate list with selections the payment rejects.
//! - CR 800.4a + CR 614.12: a player who loses to the life paid for an
//!   as-enters option (Multiversal Passage at 2 life) leaves cleanly; the game
//!   ends instead of the engine panicking while abandoning the entry's
//!   replacement continuation.
//!
//! Oracle:
//! - March of Otherworldly Light ({X}{W}, instant): "As an additional cost
//!   to cast this spell, you may exile any number of white cards from your
//!   hand. This spell costs {2} less to cast for each card exiled this way.
//!   Exile target artifact, creature, or enchantment with mana value X or
//!   less."
//! - Kozilek's Command ({X}{C}{C}, kindred instant): "Choose two — Target
//!   player creates X 0/1 colorless Eldrazi Spawn creature tokens with
//!   'Sacrifice this token: Add {C}.'; Target player scries X, then draws a
//!   card; Exile target creature with mana value X or less; Exile up to X
//!   target cards from graveyards."
//! - Deadly Precision ({B}, instant): "As an additional cost to cast this
//!   spell, pay {4} or sacrifice an artifact or creature. Destroy target
//!   creature."
//! - Psychic Frog ({U}{B}, creature): "Whenever this creature deals combat
//!   damage to a player or planeswalker, draw a card. Discard a card: Put a
//!   +1/+1 counter on this creature. Exile three cards from your graveyard:
//!   This creature gains flying until end of turn."
//! - Multiversal Passage (land): "As this land enters, choose a basic land
//!   type. Then you may pay 2 life. If you don't, it enters tapped. This land
//!   is the chosen type."

use engine::ai_support::legal_actions;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::game::scenario_db::GameScenarioDbExt;
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

use crate::support::shared_card_db as load_db;

fn pool(kind: ManaType, count: usize) -> Vec<ManaUnit> {
    vec![ManaUnit::new(kind, ObjectId(9_990), false, vec![]); count]
}

fn cast(runner: &GameRunner, object_id: ObjectId) -> GameAction {
    GameAction::CastSpell {
        object_id,
        card_id: runner.state().objects[&object_id].card_id,
        targets: vec![],
        payment_mode: CastPaymentMode::Auto,
    }
}

fn offers_cast_of(runner: &GameRunner, object_id: ObjectId) -> bool {
    legal_actions(runner.state()).iter().any(
        |action| matches!(action, GameAction::CastSpell { object_id: id, .. } if *id == object_id),
    )
}

#[test]
fn x_dependent_target_spell_without_any_target_is_not_castable() {
    let Some(db) = load_db() else {
        return;
    };
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let march = scenario.add_real_card(P0, "March of Otherworldly Light", Zone::Hand, db);
    scenario.add_real_card(P1, "Forest", Zone::Battlefield, db);
    scenario.with_mana_pool(P0, pool(ManaType::White, 6));
    let runner = scenario.build();

    assert!(
        !offers_cast_of(&runner, march),
        "no artifact, creature or enchantment exists for any X, so the cast can't be offered"
    );
}

#[test]
fn x_dependent_target_spell_offers_only_x_values_with_a_target() {
    let Some(db) = load_db() else {
        return;
    };
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let march = scenario.add_real_card(P0, "March of Otherworldly Light", Zone::Hand, db);
    scenario.add_real_card(P1, "Grizzly Bears", Zone::Battlefield, db);
    scenario.with_mana_pool(P0, pool(ManaType::White, 5));
    let mut runner = scenario.build();

    assert!(
        offers_cast_of(&runner, march),
        "a mana value 2 creature can be exiled with X >= 2"
    );
    runner.act(cast(&runner, march)).expect("cast March");
    let WaitingFor::ChooseXValue { min, max, .. } = runner.state().waiting_for.clone() else {
        panic!(
            "expected the X announcement, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(
        (min, max),
        (0, 4),
        "five white mana: {{W}} plus up to four for X"
    );
    let offered: Vec<u32> = legal_actions(runner.state())
        .into_iter()
        .filter_map(|action| match action {
            GameAction::ChooseX { value } => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(
        offered,
        vec![2, 3, 4],
        "X = 0 and 1 leave Grizzly Bears out of reach"
    );
}

fn kozilek_modes_offered(colorless: usize) -> Vec<Vec<usize>> {
    let db = load_db().expect("checked by the caller");
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let command = scenario.add_real_card(P0, "Kozilek's Command", Zone::Hand, db);
    scenario.add_real_card(P1, "Grizzly Bears", Zone::Battlefield, db);
    scenario.with_mana_pool(P0, pool(ManaType::Colorless, colorless));
    let mut runner = scenario.build();
    runner
        .act(cast(&runner, command))
        .expect("cast Kozilek's Command");
    assert!(
        matches!(runner.state().waiting_for, WaitingFor::ModeChoice { .. }),
        "expected the mode choice, got {:?}",
        runner.state().waiting_for
    );
    legal_actions(runner.state())
        .into_iter()
        .filter_map(|action| match action {
            GameAction::SelectModes { indices } => Some(indices),
            _ => None,
        })
        .collect()
}

#[test]
fn x_dependent_mode_is_unavailable_when_no_announceable_x_serves_it() {
    if load_db().is_none() {
        return;
    }
    // {C}{C} alone: X can only be 0, and no creature has mana value 0.
    let offered = kozilek_modes_offered(2);
    assert!(!offered.is_empty(), "the other modes stay castable");
    assert!(
        offered.iter().all(|modes| !modes.contains(&2)),
        "the exile-creature mode needs X >= 2 here: {offered:?}"
    );
    // Two more mana lets X reach Grizzly Bears' mana value.
    let offered = kozilek_modes_offered(4);
    assert!(
        offered.iter().any(|modes| modes.contains(&2)),
        "with X up to 2 the exile-creature mode can be chosen: {offered:?}"
    );
}

#[test]
fn losing_to_an_as_enters_life_payment_ends_the_game() {
    let Some(db) = load_db() else {
        return;
    };
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let passage = scenario.add_real_card(P0, "Multiversal Passage", Zone::Hand, db);
    scenario.with_life(P0, 2);
    let mut runner = scenario.build();

    runner
        .act(GameAction::PlayLand {
            object_id: passage,
            card_id: runner.state().objects[&passage].card_id,
        })
        .expect("play Multiversal Passage");
    // Answer every prompt before the life payment the plain way, then pay.
    for _ in 0..4 {
        match &runner.state().waiting_for {
            WaitingFor::ReplacementChoice { candidates, .. } => {
                let pay = candidates
                    .iter()
                    .position(|candidate| candidate.description.contains("life"))
                    .expect("the pay-2-life option");
                runner
                    .act(GameAction::ChooseReplacement { index: pay })
                    .expect("paying 2 life at 2 life is legal (CR 119.4)");
            }
            WaitingFor::NamedChoice { options, .. } => {
                let choice = options[0].clone();
                runner
                    .act(GameAction::ChooseOption { choice })
                    .expect("choose a basic land type");
            }
            _ => break,
        }
    }
    assert!(
        matches!(runner.state().waiting_for, WaitingFor::GameOver { winner: Some(winner) } if winner == P1),
        "P0 is at 0 life and loses (CR 704.5a), got {:?}",
        runner.state().waiting_for
    );
}

const TREASURE: &str = "{T}, Sacrifice this token: Add one mana of any color.";

#[test]
fn sacrifice_cost_spares_the_only_source_of_the_mana_still_owed() {
    let Some(db) = load_db() else {
        return;
    };
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let precision = scenario.add_real_card(P0, "Deadly Precision", Zone::Hand, db);
    let treasure = scenario
        .add_artifact_from_oracle(P0, "Treasure", TREASURE)
        .with_subtypes(vec!["Treasure"])
        .id();
    let fodder = scenario.add_real_card(P0, "Grizzly Bears", Zone::Battlefield, db);
    let victim = scenario.add_real_card(P1, "Grizzly Bears", Zone::Battlefield, db);
    let mut runner = scenario.build();

    runner
        .act(cast(&runner, precision))
        .expect("cast Deadly Precision");
    for _ in 0..4 {
        match &runner.state().waiting_for {
            WaitingFor::TargetSelection { .. } => {
                runner
                    .act(GameAction::ChooseTarget {
                        target: Some(TargetRef::Object(victim)),
                    })
                    .expect("target the opposing creature");
            }
            WaitingFor::PayCost { .. } => break,
            other => panic!("unexpected prompt before the sacrifice: {other:?}"),
        }
    }
    let offered: Vec<Vec<ObjectId>> = legal_actions(runner.state())
        .into_iter()
        .filter_map(|action| match action {
            GameAction::SelectCards { cards } => Some(cards),
            _ => None,
        })
        .collect();
    assert!(
        offered.iter().any(|cards| cards == &vec![fodder]),
        "sacrificing the creature leaves the Treasure for {{B}}: {offered:?}"
    );
    assert!(
        offered.iter().all(|cards| !cards.contains(&treasure)),
        "the Treasure is the only way to pay {{B}}, so it can't be the sacrifice: {offered:?}"
    );
}

#[test]
fn exact_count_exile_cost_offers_only_that_many_cards() {
    let Some(db) = load_db() else {
        return;
    };
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let frog = scenario.add_real_card(P0, "Psychic Frog", Zone::Battlefield, db);
    // More cards than the AI's selection pool, as in the game that looped.
    for _ in 0..14 {
        scenario.add_real_card(P0, "Swamp", Zone::Graveyard, db);
    }
    let mut runner = scenario.build();

    runner
        .act(GameAction::ActivateAbility {
            source_id: frog,
            ability_index: 1,
        })
        .expect("activate the exile-three ability");
    let WaitingFor::PayCost {
        count, min_count, ..
    } = runner.state().waiting_for.clone()
    else {
        panic!(
            "expected the exile cost prompt, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(
        (min_count, count),
        (3, 3),
        "the cost exiles exactly three cards"
    );
    let offered: Vec<Vec<ObjectId>> = legal_actions(runner.state())
        .into_iter()
        .filter_map(|action| match action {
            GameAction::SelectCards { cards } => Some(cards),
            _ => None,
        })
        .collect();
    assert!(!offered.is_empty(), "some three-card selection is offered");
    assert!(
        offered.iter().all(|cards| cards.len() == 3),
        "every offered selection pays the cost: {offered:?}"
    );
}
