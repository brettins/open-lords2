

#[path = "../common/mod.rs"]
mod common;


fn install() -> Option<String> {
    l2_testkit::install_dir().map(|d| d.display().to_string())
}

macro_rules! skip_without_install {
    () => {
        match install() {
            Some(d) => d,
            None => {
                l2_testkit::skip!("LORDS2_DIR not set or not a directory - skipping corpus test");
            }
        }
    };
}

mod indexing;
mod seeding;
mod difficulty;


