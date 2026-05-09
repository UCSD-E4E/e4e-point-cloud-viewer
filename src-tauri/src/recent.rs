//! Recent-files list logic. Pure function for testability;
//! persistence and IPC live in `lib.rs`.

#[must_use]
pub fn add_recent(mut list: Vec<String>, entry: String, max: usize) -> Vec<String> {
    list.retain(|p| p != &entry);
    list.insert(0, entry);
    list.truncate(max);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn new_entry_goes_to_front() {
        assert_eq!(
            add_recent(s(&["a", "b"]), "c".into(), 5),
            s(&["c", "a", "b"]),
        );
    }

    #[test]
    fn duplicate_moves_to_front_without_appearing_twice() {
        assert_eq!(
            add_recent(s(&["a", "b", "c"]), "b".into(), 5),
            s(&["b", "a", "c"]),
        );
    }

    #[test]
    fn truncates_to_max_keeping_most_recent() {
        assert_eq!(
            add_recent(s(&["a", "b", "c"]), "d".into(), 2),
            s(&["d", "a"])
        );
    }

    #[test]
    fn adding_to_empty_list_yields_singleton() {
        assert_eq!(add_recent(vec![], "x".into(), 5), s(&["x"]));
    }
}
