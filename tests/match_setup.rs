use tracker_core::session::{CourtEnd, MatchFormat, MatchSetup, Player};

fn setup() -> MatchSetup {
    MatchSetup {
        players: vec![
            Player {
                id: "p1".into(),
                name: "Piotr".into(),
                genitive: Some("Piotra".into()),
                start_end: CourtEnd::North,
            },
            Player {
                id: "p2".into(),
                name: "Marek".into(),
                genitive: Some("Marka".into()),
                start_end: CourtEnd::South,
            },
        ],
        first_server: "p1".into(),
        format: MatchFormat::SinglesAdTiebreak,
    }
}

#[test]
fn default_singles_is_valid_and_anonymous() {
    let default = MatchSetup::default_singles();
    assert!(default.validate().is_ok());
    assert_eq!(default.players.len(), 2);
    assert_eq!(default.players[0].name, "Player 1");
    assert_eq!(default.players[1].name, "Player 2");
    // Nazwy domyslne sa w jezyku bazowym (angielskim); UI podmienia je
    // na zlokalizowane przy wyswietlaniu, nie w rdzeniu.
    assert!(default.players.iter().all(|p| p.genitive.is_none()));
    assert_ne!(default.players[0].start_end, default.players[1].start_end);
}

#[test]
fn valid_setup_passes() {
    assert!(setup().validate().is_ok());
}

#[test]
fn rejects_duplicate_player_ids() {
    let mut s = setup();
    s.players[1].id = "p1".into();
    s.first_server = "p1".into();
    assert!(s.validate().is_err());
}

#[test]
fn rejects_unknown_first_server() {
    let mut s = setup();
    s.first_server = "p9".into();
    assert!(s.validate().is_err());
}

#[test]
fn rejects_both_players_on_the_same_end() {
    let mut s = setup();
    s.players[1].start_end = CourtEnd::North;
    assert!(s.validate().is_err());
}

#[test]
fn rejects_wrong_player_count_for_singles() {
    let mut s = setup();
    s.players.pop();
    assert!(s.validate().is_err());
}

#[test]
fn player_lookup_by_id() {
    let s = setup();
    assert_eq!(s.player("p2").map(|p| p.name.as_str()), Some("Marek"));
    assert!(s.player("p9").is_none());
}

/// Zmiana stron nastepuje po nieparzystych gemach: po 1., 3., 5. …
#[test]
fn ends_swap_after_odd_games() {
    let s = setup();
    assert_eq!(s.end_of("p1", 0), Some(CourtEnd::North));
    assert_eq!(s.end_of("p1", 1), Some(CourtEnd::South));
    assert_eq!(s.end_of("p1", 2), Some(CourtEnd::South));
    assert_eq!(s.end_of("p1", 3), Some(CourtEnd::North));
    assert_eq!(s.end_of("p1", 4), Some(CourtEnd::North));
    assert_eq!(s.end_of("p1", 5), Some(CourtEnd::South));
}

#[test]
fn both_players_always_face_each_other() {
    let s = setup();
    for games in 0..12u32 {
        let a = s.end_of("p1", games).unwrap();
        let b = s.end_of("p2", games).unwrap();
        assert_ne!(
            a, b,
            "po {games} gemach gracze staneli po tej samej stronie"
        );
    }
}

/// Dopelniacz jest opcjonalny — jego brak nie moze blokowac niczego,
/// bo oglaszanie domyslnie uzywa konstrukcji w mianowniku.
#[test]
fn genitive_is_optional_and_does_not_affect_validation() {
    let mut s = setup();
    assert_eq!(
        s.player("p1").and_then(|p| p.genitive.as_deref()),
        Some("Piotra")
    );

    s.players[0].genitive = None;
    assert!(s.validate().is_ok());
}

#[test]
fn court_end_has_an_opposite() {
    assert_eq!(CourtEnd::North.opposite(), CourtEnd::South);
    assert_eq!(CourtEnd::South.opposite(), CourtEnd::North);
}

#[test]
fn format_string_matches_manifest_vocabulary() {
    assert_eq!(
        MatchFormat::SinglesAdTiebreak.as_str(),
        "singles-ad-tiebreak"
    );
}
