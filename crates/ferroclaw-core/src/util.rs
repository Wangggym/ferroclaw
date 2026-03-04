/// Expand a leading `~/` to the user's home directory.
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{}{}", home, &path[1..])
    } else {
        path.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_with_tilde_prefix() {
        std::env::set_var("HOME", "/home/testuser");
        let result = expand_tilde("~/Documents/file.txt");
        assert_eq!(result, "/home/testuser/Documents/file.txt");
    }

    #[test]
    fn expand_tilde_without_tilde_prefix() {
        let result = expand_tilde("/absolute/path/file.txt");
        assert_eq!(result, "/absolute/path/file.txt");
    }

    #[test]
    fn expand_tilde_relative_path() {
        let result = expand_tilde("relative/path");
        assert_eq!(result, "relative/path");
    }
}
