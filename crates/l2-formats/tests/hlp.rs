use l2_formats::hlp::{Hlp, Topic};

fn help() -> Option<Hlp> {
    let dir = l2_testkit::install_dir()?;
    let bytes = std::fs::read(dir.join("L2help.hlp")).ok()?;
    Some(Hlp::parse(&bytes).expect("minor 33, flags 0, no phrases"))
}

fn links(t: &Topic) -> Vec<(String, u32)> {
    t.lines.iter().flat_map(|l| &l.spans).filter_map(|s| Some((s.text.clone(), s.jump?))).collect()
}

/// Opt_GameHelpContents 0x00434942: WinHelpA(hwnd, "l2help.hlp", HELP_CONTENTS, 1).
/// Ablate `TL_TOPIC_HEADER if title.is_some() => break` and topic 1's text runs on.
#[test]
fn the_help_files_contents_topic_reads_as_its_list_of_chapters() {
    let Some(hlp) = help() else { l2_testkit::skip!("no install, or no L2help.hlp (the DOS release)") };
    assert_eq!(hlp.title, "Lords of the Realm II");
    assert_eq!(hlp.contents, 0, "|SYSTEM record 3");
    assert_eq!(hlp.topic_count(), 170);
    let t = hlp.topic_at_offset(hlp.contents).expect("the contents topic");
    assert_eq!(t.lines.first().map(|l| l.text()).as_deref(), Some("LORDS of the REALM II"));
    assert_eq!(t.lines.last().map(|l| l.text()).as_deref(), Some("For help with Help, press the F1 key."));
    let names: Vec<String> = links(&t).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names.len(), 13, "{names:?}");
    for chapter in ["Frequently Asked Questions", "Getting Started", "Battles", "Diplomacy", "Credits"] {
        assert!(names.iter().any(|n| n == chapter), "{chapter} in {names:?}");
    }
}

/// Ablate the `0xE2 | 0xE3 | 0xE6 | 0xE7 => Cmd::Jump` arm and no span carries a hash;
/// ablate the `|CONTEXT` read and `jump` finds nothing.
#[test]
fn every_contents_link_opens_its_chapter_and_battles_is_about_battles() {
    let Some(hlp) = help() else { l2_testkit::skip!("no install, or no L2help.hlp (the DOS release)") };
    let t = hlp.topic_at_offset(hlp.contents).unwrap();
    for (name, hash) in links(&t) {
        let off = hlp.jump(hash).unwrap_or_else(|| panic!("{name}: hash {hash:#x} not in |CONTEXT"));
        let chapter = hlp.topic_at_offset(off).unwrap_or_else(|| panic!("{name}: {off:#x}"));
        assert_ne!(off, hlp.contents, "{name}");
        if name == "Battles" {
            let text: String = chapter.lines.iter().map(|l| l.text()).collect();
            assert!(text.contains("battle") || text.contains("Battle"), "{text}");
        }
    }
}
