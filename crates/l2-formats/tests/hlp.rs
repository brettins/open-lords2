use l2_formats::hlp::Hlp;

/// Opt_GameHelpContents 0x00434942: WinHelpA(hwnd, "l2help.hlp", HELP_CONTENTS, 1).
/// Ablate `TL_TOPIC_HEADER if title.is_some() => break` and topic 1's text runs on.
#[test]
fn the_help_files_contents_topic_reads_as_its_list_of_chapters() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no install");
    };
    let Ok(bytes) = std::fs::read(dir.join("L2help.hlp")) else {
        l2_testkit::skip!("no L2help.hlp (the DOS release ships none)");
    };
    let hlp = Hlp::parse(&bytes).expect("minor 33, flags 0, no phrases");
    assert_eq!(hlp.title, "Lords of the Realm II");
    assert_eq!(hlp.contents, 0, "|SYSTEM record 3");
    let t = hlp.topic_at_offset(hlp.contents).expect("the contents topic");
    assert_eq!(t.lines.first().map(String::as_str), Some("LORDS of the REALM II"));
    for chapter in ["Frequently Asked Questions", "Getting Started", "Diplomacy", "Credits"] {
        assert!(t.lines.iter().any(|l| l == chapter), "{chapter} in {:?}", t.lines);
    }
    assert_eq!(t.lines.last().map(String::as_str), Some("For help with Help, press the F1 key."));
}
