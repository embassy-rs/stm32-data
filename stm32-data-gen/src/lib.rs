pub mod chips;
pub mod dma;
pub mod docs;
pub mod gpio_af;
pub mod header;
pub mod interrupts;
pub mod memory;
pub mod normalize_peris;
pub mod rcc;
pub mod registers;
pub mod util;

#[macro_export]
macro_rules! regex {
    ($re:literal) => {{
        ::ref_thread_local::ref_thread_local! {
            static managed REGEX: ::regex::Regex = ::regex::Regex::new($re).unwrap();
        }
        <REGEX as ::ref_thread_local::RefThreadLocal<::regex::Regex>>::borrow(&REGEX)
    }};
}
