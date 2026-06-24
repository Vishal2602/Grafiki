pub fn new_ulid() -> String {
    ulid::Ulid::new().to_string()
}

#[cfg(test)]
mod tests {
    use super::new_ulid;

    #[test]
    fn generated_ulids_are_26_characters() {
        assert_eq!(new_ulid().len(), 26);
    }

    #[test]
    fn generated_ulids_are_parseable() {
        let id = new_ulid();

        assert!(ulid::Ulid::from_string(&id).is_ok());
    }
}
