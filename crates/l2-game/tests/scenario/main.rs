

use std::path::PathBuf;

use l2_mods::Platform;

fn platform(dir: &PathBuf) -> Platform {
    Platform::builder().base(dir).build().expect("the install mounts")
}

macro_rules! game {
    () => {{
        let save = l2_testkit::england!();
        scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads")
    }};
}

macro_rules! install {
    () => {
        l2_testkit::install!()
    };
}

mod england_fixture;
mod turn_execution;
mod save_loading;

