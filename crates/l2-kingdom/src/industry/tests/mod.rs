
mod output;
mod harvest;
mod wages;
mod castles;

use super::*;

#[allow(dead_code)]
const Q: Quirks = Quirks::FAITHFUL;

const T: &Tables = &Tables::DEFAULT;

fn worker_county(job: usize, workers: i32) -> County {
    let mut c = County::new();
    c.labour[job] = workers;
    c
}

fn advanced_county(workers: i32) -> County {
    let mut c = County::new();
    for cm in Commodity::ALL {
        c.labour[cm.job()] = workers;
        c.industry[cm.index()].efficiency = cm.base_efficiency();
        c.industry[cm.index()].capacity = workers;
    }
    c
}


