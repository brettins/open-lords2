
#[macro_export]
macro_rules! chain_neighbours {
    ($kingdom:expr) => {{
        let k = &mut $kingdom;
        let n = k.county_count;
        for id in 1..=n {
            let c = &mut k.counties[id];
            c.neighbour_count = 0;
            if id > 1 {
                c.add_neighbour(id as u8 - 1);
            }
            if id < n {
                c.add_neighbour(id as u8 + 1);
            }
        }
    }};
}

#[macro_export]
macro_rules! isolated_counties {
    ($kingdom:expr) => {{
        let k = &mut $kingdom;
        for id in 1..=k.county_count {
            k.counties[id].neighbour_count = 0;
        }
    }};
}
