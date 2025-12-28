pub fn get_puzzle_targets() -> Vec<(u32, &'static str)> {
    vec![
        (66, "13zb1hQbWVsc2S7ZTZnP2G4undNNpdh5so"),
        (67, "1BY8GQbnueYofwSuFAT3USAhGjPrkxDdW9"),
        (68, "1MVDYgVaSN6iKKEsbzRUAYFrYJadLYZvvZ"),
        (69, "19vkiEajfhuZ8bs8Zu2jgmC6oqZbWqhxhG"),
        (71, "1PWo3JeB9jrGwfHDNpdGK54CRas7fsVzXU"),
        (72, "1JTK7s9YVYywfm5XUH7RNhHJH1LshCaRFR"),
        (73, "12VVRNPi4SJqUTsp6FmqDqY5sGosDtysn4"),
    ]
}

pub fn get_range(puzzle_num: u32) -> (u128, u128) {
    if puzzle_num > 0 && puzzle_num < 128 {
        (1u128 << (puzzle_num - 1), (1u128 << puzzle_num) - 1)
    } else {
        (0, 0)
    }
}
