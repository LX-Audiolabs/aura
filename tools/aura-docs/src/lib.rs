//! Tests for the public Markdown guides. Rustdoc compiles their Rust fences;
//! shell commands and manifest snippets still need separate review.

#[doc = include_str!("../../../README.md")]
pub struct Readme;

#[doc = include_str!("../../../CLAP-GUIDE.md")]
pub struct ClapGuide;

#[cfg(test)]
mod tests {
    #[test]
    fn readme_and_clap_guide_share_the_same_rust_example() {
        fn rust_examples(markdown: &str) -> Vec<&str> {
            markdown
                .split("```rust\n")
                .skip(1)
                .map(|block| block.split_once("\n```").expect("closed Rust fence").0)
                .collect()
        }

        let readme = include_str!("../../../README.md").replace("\r\n", "\n");
        let guide = include_str!("../../../CLAP-GUIDE.md").replace("\r\n", "\n");
        let examples = rust_examples(&readme);
        assert!(!examples.is_empty(), "README must contain a Rust example");
        assert_eq!(examples, rust_examples(&guide));
    }
}
