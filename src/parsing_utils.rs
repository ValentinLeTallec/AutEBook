use scraper::{Html, Selector};

/// Declare selectors that are only initialised once and add tests to ensure they can be safely unwraped
/// The syntax is `SELECTOR_NAME: "selector";`
#[macro_export]
macro_rules! lazy_selectors {
    ( $( $selector_name:ident: $selector:expr; )+ ) => {
        $(
        static $selector_name: std::sync::LazyLock<scraper::Selector> =
            std::sync::LazyLock::new(|| scraper::Selector::parse($selector)
                .expect("One of the lazy selectors failed, run `cargo test` to find out which"));
        )*

        #[cfg(test)]
        mod lazy_selectors_autotest {
            $(
                /// Ensure the selector can be unwraped safely
                #[test]
                #[allow(non_snake_case)]
                fn $selector_name() {
                    assert!(scraper::Selector::parse(&$selector).is_ok());
                }
            )*
        }
    };
}

pub trait QuickSelect {
    fn get_inner_html_of(&self, selector: &Selector) -> Option<String>;
    fn get_meta_content_of(&self, selector: &Selector) -> Option<String>;
}
impl QuickSelect for Html {
    fn get_inner_html_of(&self, selector: &Selector) -> Option<String> {
        self.select(selector)
            .next()
            .map(|element| element.inner_html())
            .filter(|s| !s.is_empty())
    }
    fn get_meta_content_of(&self, selector: &Selector) -> Option<String> {
        self.select(selector)
            .next()
            .and_then(|e| e.attr("content"))
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
    }
}
